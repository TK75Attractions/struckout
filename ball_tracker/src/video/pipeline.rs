//! 1 台のカメラの動画を ffmpeg でデコード・縮小し、解析して、検出を送り出す。
//!
//! スレッドは 2 本使う。
//! - 書き込み: 受け取った H.264 を ffmpeg の標準入力に書く
//! - 解析: ffmpeg の標準出力から縮小した画像を読み、[`Analyzer`] にかける
//!
//! ffmpeg の出力には時刻が付かないので、フレームは書いた順に出てくるものとして、書いたときの時刻を順に割り当てる。
//! カメラアプリは B フレームを使わない (デコードの順と表示の順が同じ) ので、この対応は崩れない。
//! キーフレームより前のフレームはデコードできず、出てこないので書かない。

use std::{
    collections::VecDeque,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc::{self as std_mpsc, SyncSender, TrySendError},
    },
    thread,
    time::{Duration, Instant},
};

use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use struckout_proto::{DetectionsPacket, VideoFrame};
use tokio::sync::mpsc;
use tracing::{info, warn};

use super::{
    VideoOptions,
    analysis::{AnalysisSettings, Analyzer},
};

/// ffmpeg に書く前にためておけるフレームの数 (120fps で 2 秒)。超えたら次のキーフレームまで捨てる。
const INPUT_QUEUE: usize = 240;
/// 解析の状況をログに出す間隔。
const STATS_INTERVAL: Duration = Duration::from_secs(10);

/// 受け取ったフレームの時刻。ffmpeg から出てきたフレームに順に割り当てる。
#[derive(Debug, Clone, Copy)]
struct FrameMeta {
    sensor_timestamp_ns: i64,
    timestamp_ms: i64,
    received_at: DateTime<Utc>,
    /// 受け取った時刻。段ごとの時間は、時計の補正の影響を受けない単調な時計で測る
    received: Instant,
}

/// ffmpeg に書くもの。`frame` は、フレームなら `Some`、設定 (SPS/PPS) なら `None`。
struct Input {
    data: Vec<u8>,
    frame: Option<FrameMeta>,
}

/// ffmpeg に書いたフレーム。
#[derive(Debug, Clone, Copy)]
struct WrittenFrame {
    meta: FrameMeta,
    /// 書き込みのスレッドが ffmpeg に書き始めた時刻
    handed: Instant,
}

/// 受け取ってから解析するまでのフレーム。受信、書き込み、解析のスレッドで共有する。
#[derive(Default)]
struct FrameQueue {
    /// ffmpeg への書き込みを待っているフレームの数
    waiting: AtomicUsize,
    /// ffmpeg に書いて、まだ出てきていないフレーム (書いた順)
    written: Mutex<VecDeque<WrittenFrame>>,
}

pub struct Pipeline {
    camera_id: u32,
    width: usize,
    height: usize,
    input: Option<SyncSender<Input>>,
    queue: Arc<FrameQueue>,
    settings: Arc<Mutex<AnalysisSettings>>,
    child: Child,
    /// キーフレームを待っている (まだ書き始めていないか、詰まって捨てたあと)
    waiting_for_key_frame: bool,
    /// 直近の設定 (SPS/PPS)。キーフレームの前に書く
    codec_config: Option<Vec<u8>>,
}

impl Pipeline {
    /// ffmpeg を起動する。`out` には、検出のあったフレームの [`DetectionsPacket`] を、受け取った時刻と一緒に送る。
    pub fn spawn(
        options: &VideoOptions,
        settings: AnalysisSettings,
        out: mpsc::Sender<(DateTime<Utc>, DetectionsPacket)>,
    ) -> std::io::Result<Self> {
        let (width, height) = (settings.width, settings.height);
        let camera_id = settings.camera_id;
        // GPU でデコードするときは、入力の前に方式を指定する。デコードした画像は ffmpeg が CPU 側に戻してから縮める
        let hwaccel: Vec<&str> = options
            .hwaccel
            .as_deref()
            .map(|name| vec!["-hwaccel", name])
            .unwrap_or_default();
        let mut child = Command::new(&options.ffmpeg)
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                // 遅れは 60fps の入力で 40ms 前後 (2026-10-15 に測った)。どれも測って決めた:
                // - low_delay がないと、並べ替えに備えて 250ms ほどためる
                // - probesize を小さくすると、最初のフレームが出るまでが 0.9 秒から 0.08 秒になる
                // - "-fflags nobuffer" は付けない。付けると 1 秒 (60 フレーム) 遅れたまま戻らない
                "-flags",
                "low_delay",
                "-probesize",
                "32768",
                // フレーム単位の並列化は、その枚数ぶん出てくるのが遅れるので使わない
                "-threads",
                "1",
            ])
            .args(&hwaccel)
            .args([
                "-f",
                "h264",
                "-i",
                "pipe:0",
                // アプリの GPU での縮小 (2x2 の平均) に近い
                "-vf",
                &format!("scale={width}:{height}:flags=area"),
                "-fps_mode",
                "passthrough",
                "-f",
                "rawvideo",
                "-pix_fmt",
                "rgb24",
                "pipe:1",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;
        let mut stdin = child.stdin.take().expect("piped stdin");
        let mut stdout = child.stdout.take().expect("piped stdout");

        let (input, input_rx) = std_mpsc::sync_channel::<Input>(INPUT_QUEUE);
        let queue = Arc::new(FrameQueue::default());
        {
            let queue = Arc::clone(&queue);
            thread::Builder::new()
                .name(format!("video-write-{camera_id}"))
                .spawn(move || {
                    while let Ok(Input { data, frame }) = input_rx.recv() {
                        // 時刻は書く前に積む (ffmpeg の出力より先に積まれているように)
                        if let Some(meta) = frame {
                            queue.written.lock().push_back(WrittenFrame {
                                meta,
                                handed: Instant::now(),
                            });
                            queue.waiting.fetch_sub(1, Ordering::Relaxed);
                        }
                        if stdin.write_all(&data).and_then(|()| stdin.flush()).is_err() {
                            break;
                        }
                    }
                })?;
        }

        let settings = Arc::new(Mutex::new(settings));
        {
            let queue = Arc::clone(&queue);
            let settings = Arc::clone(&settings);
            let snapshot_dir = options.snapshot_dir.clone();
            let detection = options.detection;
            thread::Builder::new()
                .name(format!("video-analyze-{camera_id}"))
                .spawn(move || {
                    analyze_loop(
                        camera_id,
                        &mut stdout,
                        width,
                        height,
                        &queue,
                        &settings,
                        snapshot_dir,
                        detection,
                        &out,
                    );
                })?;
        }
        info!(
            camera_id,
            width,
            height,
            hwaccel = options.hwaccel.as_deref().unwrap_or("none"),
            "started analyzing video"
        );
        Ok(Self {
            camera_id,
            width,
            height,
            input: Some(input),
            queue,
            settings,
            child,
            waiting_for_key_frame: true,
            codec_config: None,
        })
    }

    /// 解析画像の大きさが同じなら、設定 (範囲、視線の計算) だけ差し替える。違えば `false` を返す。
    pub fn update_settings(&self, settings: AnalysisSettings) -> bool {
        if settings.width != self.width || settings.height != self.height {
            return false;
        }
        *self.settings.lock() = settings;
        true
    }

    /// 受け取ったフレームを ffmpeg に渡す。
    pub fn push(&mut self, frame: VideoFrame, received_at: DateTime<Utc>) {
        if frame.codec_config {
            self.codec_config = Some(frame.data.clone());
            if !self.waiting_for_key_frame {
                self.write(frame.data, None);
            }
            return;
        }
        if self.waiting_for_key_frame {
            if !frame.key_frame {
                return;
            }
            if let Some(config) = self.codec_config.clone() {
                self.write(config, None);
            }
            self.waiting_for_key_frame = false;
        }
        let meta = FrameMeta {
            sensor_timestamp_ns: frame.sensor_timestamp_ns,
            timestamp_ms: frame.timestamp,
            received_at,
            received: Instant::now(),
        };
        self.write(frame.data, Some(meta));
    }

    fn write(&mut self, data: Vec<u8>, frame: Option<FrameMeta>) {
        let Some(input) = &self.input else {
            return;
        };
        // 書き込みのスレッドが減らすより先に数える (数が 0 を下回らないように)
        if frame.is_some() {
            self.queue.waiting.fetch_add(1, Ordering::Relaxed);
        }
        let result = input.try_send(Input { data, frame });
        if result.is_err() && frame.is_some() {
            self.queue.waiting.fetch_sub(1, Ordering::Relaxed);
        }
        match result {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                warn!(
                    camera_id = self.camera_id,
                    "video decoder is falling behind; waiting for the next key frame"
                );
                self.waiting_for_key_frame = true;
            }
            Err(TrySendError::Disconnected(_)) => {
                warn!(camera_id = self.camera_id, "ffmpeg stopped accepting video");
                self.input = None;
            }
        }
    }
}

impl Drop for Pipeline {
    fn drop(&mut self) {
        // 標準入力を閉じると ffmpeg は終わる。念のため止める
        self.input = None;
        let _ = self.child.kill();
        let _ = self.child.wait();
        info!(camera_id = self.camera_id, "stopped analyzing video");
    }
}

fn analyze_loop(
    camera_id: u32,
    stdout: &mut impl Read,
    width: usize,
    height: usize,
    queue: &FrameQueue,
    settings: &Mutex<AnalysisSettings>,
    mut snapshot_dir: Option<PathBuf>,
    detection: super::analysis::DetectionOptions,
    out: &mpsc::Sender<(DateTime<Utc>, DetectionsPacket)>,
) {
    let mut analyzer = Analyzer::new(settings.lock().clone()).with_options(detection);
    let mut image = vec![0u8; width * height * 3];
    let mut stats = Stats::new();
    loop {
        let read_begin = Instant::now();
        if stdout.read_exact(&mut image).is_err() {
            break;
        }
        let decoded = Instant::now();
        let Some(WrittenFrame { meta, handed }) = queue.written.lock().pop_front() else {
            warn!(camera_id, "a decoded frame has no timestamp; skipping it");
            continue;
        };
        stats.add_input(
            handed - meta.received,
            decoded - handed,
            decoded - read_begin,
        );
        if let Some(dir) = snapshot_dir.take() {
            let path = dir.join(format!(
                "camera{camera_id}_{}.bmp",
                Utc::now().format("%Y%m%d-%H%M%S")
            ));
            match save_bmp(&path, &image, width, height) {
                Ok(()) => {
                    info!(camera_id, path = %path.display(), "saved the first analysis image")
                }
                Err(e) => {
                    warn!(camera_id, error = ?e, path = %path.display(), "failed to save the first analysis image")
                }
            }
        }
        {
            let current = settings.lock();
            if *current != *analyzer.settings() {
                analyzer.update(current.clone());
            }
        }
        let begin = Instant::now();
        let analyzed = analyzer.analyze(&image, meta.sensor_timestamp_ns, meta.timestamp_ms);
        stats.add(
            begin.elapsed(),
            Utc::now() - meta.received_at,
            analyzed.packet.is_some(),
            analyzed.ignored,
        );
        if let Some(packet) = analyzed.packet {
            let send_begin = Instant::now();
            if out.blocking_send((meta.received_at, packet)).is_err() {
                break;
            }
            stats.sending += send_begin.elapsed();
        }
        if stats.started.elapsed() >= STATS_INTERVAL {
            stats.log(
                camera_id,
                queue.waiting.load(Ordering::Relaxed),
                queue.written.lock().len(),
            );
            stats = Stats::new();
        }
    }
}

/// RGB (上の行から) の画像を、24 ビットの BMP で保存する。
fn save_bmp(path: &Path, rgb: &[u8], width: usize, height: usize) -> std::io::Result<()> {
    let row = (width * 3).div_ceil(4) * 4;
    let image_size = row * height;
    let to_u32 = |value: usize| u32::try_from(value).map_err(std::io::Error::other);
    let mut bytes = Vec::with_capacity(54 + image_size);
    bytes.extend_from_slice(b"BM");
    bytes.extend_from_slice(&to_u32(54 + image_size)?.to_le_bytes());
    bytes.extend_from_slice(&[0; 4]);
    bytes.extend_from_slice(&54u32.to_le_bytes());
    bytes.extend_from_slice(&40u32.to_le_bytes());
    bytes.extend_from_slice(&to_u32(width)?.to_le_bytes());
    bytes.extend_from_slice(&to_u32(height)?.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&24u16.to_le_bytes());
    bytes.extend_from_slice(&[0; 4]); // 圧縮なし
    bytes.extend_from_slice(&to_u32(image_size)?.to_le_bytes());
    bytes.extend_from_slice(&[0; 16]);
    // BMP は下の行から、画素は BGR の順
    for y in (0..height).rev() {
        let (chunks, remainder) = rgb[y * width * 3..(y + 1) * width * 3].as_chunks::<3>();
        debug_assert!(remainder.is_empty());
        for pixel in chunks {
            bytes.extend_from_slice(&[pixel[2], pixel[1], pixel[0]]);
        }
        bytes.resize(bytes.len() + row - width * 3, 0);
    }
    std::fs::create_dir_all(path.parent().unwrap_or(Path::new(".")))?;
    std::fs::write(path, bytes)
}

struct Stats {
    started: Instant,
    frames: u32,
    with_detections: u32,
    ignored: usize,
    analysis: Duration,
    /// 受け取ってから解析し終えるまで (デコードの待ちを含む)。フレームごと (ms)
    delays_ms: Vec<i64>,
    /// 受け取ってから ffmpeg に書き始めるまで。フレームごと
    inputs: Vec<Duration>,
    /// ffmpeg に書き始めてから、縮小した画像を読み終えるまで。フレームごと。解析が遅れて ffmpeg の出力が
    /// 詰まっているあいだも含む
    decodes: Vec<Duration>,
    /// 解析のスレッドが ffmpeg の出力を待っていた時間の合計
    waiting: Duration,
    /// 解析のスレッドが検出を追跡に渡すのを待っていた時間の合計
    sending: Duration,
}

impl Stats {
    fn new() -> Self {
        Self {
            started: Instant::now(),
            frames: 0,
            with_detections: 0,
            ignored: 0,
            analysis: Duration::ZERO,
            delays_ms: Vec::new(),
            inputs: Vec::new(),
            decodes: Vec::new(),
            waiting: Duration::ZERO,
            sending: Duration::ZERO,
        }
    }

    /// ffmpeg から出てきた 1 フレームの、そこまでの時間を足す。
    fn add_input(&mut self, input: Duration, decode: Duration, waiting: Duration) {
        self.inputs.push(input);
        self.decodes.push(decode);
        self.waiting += waiting;
    }

    fn add(
        &mut self,
        analysis: Duration,
        delay: chrono::TimeDelta,
        detected: bool,
        ignored: usize,
    ) {
        self.frames += 1;
        self.with_detections += u32::from(detected);
        self.ignored += ignored;
        self.analysis += analysis;
        self.delays_ms.push(delay.num_milliseconds());
    }

    /// `waiting` は ffmpeg への書き込みを待っているフレームの数、`written` は ffmpeg に書いてまだ出てきていない
    /// フレームの数。
    fn log(&mut self, camera_id: u32, waiting: usize, written: usize) {
        let seconds = self.started.elapsed().as_secs_f64();
        let frames = f64::from(self.frames.max(1));
        let mean_ms = |total: Duration| format!("{:.2}", total.as_secs_f64() * 1000.0 / frames);
        let ms = |duration: Duration| format!("{:.1}", duration.as_secs_f64() * 1000.0);
        self.delays_ms.sort_unstable();
        self.inputs.sort_unstable();
        self.decodes.sort_unstable();
        info!(
            camera_id,
            fps = format!("{:.1}", f64::from(self.frames) / seconds),
            frames_with_detections = self.with_detections,
            ignored_detections = self.ignored,
            analysis_ms = mean_ms(self.analysis),
            delay_ms_p50 = percentile(&self.delays_ms, 0.5),
            delay_ms_p95 = percentile(&self.delays_ms, 0.95),
            max_delay_ms = self.delays_ms.last().copied().unwrap_or_default(),
            queued = waiting + written,
            input_ms_p95 = ms(percentile(&self.inputs, 0.95)),
            ffmpeg_ms_p50 = ms(percentile(&self.decodes, 0.5)),
            ffmpeg_ms_p95 = ms(percentile(&self.decodes, 0.95)),
            queued_in_ffmpeg = written,
            wait_ms = mean_ms(self.waiting),
            send_ms = mean_ms(self.sending),
            "video analysis"
        );
    }
}

/// 小さい順に並べた値の、割合 `p` (0〜1) の位置にある値。空なら既定値 (0)。
fn percentile<T: Copy + Default>(sorted: &[T], p: f64) -> T {
    sorted
        .get(((sorted.len() as f64 - 1.0) * p).round() as usize)
        .copied()
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bmp_rows_are_padded_and_stored_bottom_up() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("snapshot.bmp");
        // 幅 1 (1 行 3 バイト -> 4 バイト)、高さ 2。上が赤、下が青
        save_bmp(&path, &[255, 0, 0, 0, 0, 255], 1, 2).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(bytes.len(), 54 + 8);
        assert_eq!(&bytes[0..2], b"BM");
        assert_eq!(&bytes[54..], &[255, 0, 0, 0, 0, 0, 255, 0]);
    }

    #[test]
    fn percentile_picks_the_nearest_rank_and_defaults_to_zero() {
        let sorted: Vec<i64> = (1..=21).collect();
        assert_eq!(percentile(&sorted, 0.5), 11);
        assert_eq!(percentile(&sorted, 0.95), 20);
        assert_eq!(percentile(&sorted, 1.0), 21);
        assert_eq!(percentile::<Duration>(&[], 0.95), Duration::ZERO);
    }
}
