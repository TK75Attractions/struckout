//! 1 台のカメラの動画を ffmpeg でデコードし、Rust で縮小・RGB 変換・解析して、検出を送り出す。
//!
//! スレッドは 2 本使う。
//! - 書き込み: 受け取った H.264 を ffmpeg の標準入力に書く
//! - 解析: ffmpeg の標準出力から NV12 を読み、縮小・RGB 変換して、[`Analyzer`] にかける
//!
//! ffmpeg の出力には時刻が付かないので、フレームは書いた順に出てくるものとして、書いたときの時刻を順に割り当てる。
//! カメラアプリは B フレームを使わない (デコードの順と表示の順が同じ) ので、この対応は崩れない。
//! キーフレームより前のフレームはデコードできず、出てこないので書かない。
//! 遅れた入力は次の新しいキーフレームまで捨て、ffmpeg に残った古い画像も解析せずに読み流して追いつく。

use std::{
    collections::VecDeque,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicU64, AtomicUsize, Ordering},
        mpsc::{self as std_mpsc, Receiver, SyncSender, TrySendError},
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
    decode::{DecodeLayout, ffmpeg_args},
};

/// ffmpeg に書く前にためておけるフレームの数 (120fps で 2 秒)。超えたら次のキーフレームまで捨てる。
const INPUT_QUEUE: usize = 240;
/// 着弾の計算にほぼ役に立たないフレームを捨てるための、受信からの遅れの上限。
/// 撮影から PC に届くまで約 0.1 秒かかり、追跡は撮影から
/// [`RAY_LIFETIME`](crate::tracking::RAY_LIFETIME) (0.5 秒) を過ぎた視線を捨てる。
/// 軌跡を作るにはその前の約 0.1 秒の視線も要るため、受信から 0.3 秒を過ぎたフレームは使わない。
const MAX_DELAY: Duration = Duration::from_millis(300);
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
    key_frame: bool,
}

/// ffmpeg に書くもの。`frame` は、フレームなら `Some`、設定 (SPS/PPS) なら `None`。
struct Input {
    data: Vec<u8>,
    frame: Option<FrameMeta>,
}

/// 書き込みを再開した回数。解析のスレッドが捨てた画像が、いま書いている区間のものかを見分ける。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Generation(u64);

/// 遅れたフレームを捨てて追いつく。書き込みのスレッドが、ffmpeg に書く直前に使う。
///
/// 遅れたまま順に処理し続けると、[`MAX_DELAY`] を超えたフレームを解析し続けることになり、遅れが戻らない。
/// P フレームは前のフレームを参照するので、1 枚捨てたら次のキーフレームまでまとめて捨てる (途中から書くと
/// ffmpeg は参照の欠けた画像を出す)。キーフレームは 1 秒おき (カメラアプリの `VideoRelay`) なので、捨てる
/// のは長くて 1 秒と少し。
#[derive(Default)]
struct CatchUp {
    /// 直近の設定 (SPS/PPS)。再開するキーフレームの前に書く
    codec_config: Option<Vec<u8>>,
    /// 捨て始めてから捨てたフレームの数。0 なら捨てていない
    dropped: usize,
    /// いま書いている区間。キーフレームから書き直すたびに増やす
    generation: Generation,
}

/// [`CatchUp::prepare`] の判断。
#[derive(Debug, PartialEq, Eq)]
enum WriteAction {
    /// 書く。`config` があれば先に書く。`resumed_after` は、再開するまでに捨てたフレームの数 (再開でなければ 0)
    Write {
        config: Option<Vec<u8>>,
        resumed_after: usize,
    },
    /// 捨てる。捨て始めたフレームなら、その遅れを `started_with_delay` に入れる (ログに 1 回だけ出すため)
    Skip {
        started_with_delay: Option<Duration>,
    },
}

impl CatchUp {
    /// `input` を書くか捨てるかを決める。`now` は書き込みのスレッドが受け取った時刻。
    ///
    /// `stale` は、解析のスレッドが遅れて捨てた画像の区間 (なければ `None`)。いま書いている区間の画像が
    /// 捨てられたら、ffmpeg の中で遅れが積もっているので、書く前には遅れていなくても捨て始める。そうしないと、
    /// デコードしてから捨てるだけのフレームを書き続けて、遅れが戻らない (CPU を 1 コアに絞った試験で、
    /// 1 秒に 120 枚のうち 60 枚ほどをデコードしてから捨てていた)。前の区間の画像が捨てられても、いまの
    /// 区間には関係ないので気にしない。
    fn prepare(&mut self, input: &Input, now: Instant, stale: Option<Generation>) -> WriteAction {
        let Some(meta) = input.frame else {
            self.codec_config = Some(input.data.clone());
            return if self.dropped > 0 {
                WriteAction::Skip {
                    started_with_delay: None,
                }
            } else {
                WriteAction::Write {
                    config: None,
                    resumed_after: 0,
                }
            };
        };
        let delay = now - meta.received;
        let fresh_key_frame = meta.key_frame && delay <= MAX_DELAY;
        if self.dropped > 0 {
            if !fresh_key_frame {
                self.dropped += 1;
                return WriteAction::Skip {
                    started_with_delay: None,
                };
            }
            self.generation.0 += 1;
            return WriteAction::Write {
                config: self.codec_config.clone(),
                resumed_after: std::mem::take(&mut self.dropped),
            };
        }
        if delay > MAX_DELAY || stale == Some(self.generation) {
            if fresh_key_frame {
                // 捨てなくても、ここから新しい区間として書き直せる
                self.generation.0 += 1;
            } else {
                self.dropped = 1;
                return WriteAction::Skip {
                    started_with_delay: Some(delay),
                };
            }
        }
        WriteAction::Write {
            config: None,
            resumed_after: 0,
        }
    }
}

/// ffmpeg に書いたフレーム。
#[derive(Debug, Clone, Copy)]
struct WrittenFrame {
    meta: FrameMeta,
    /// 書き込みのスレッドが ffmpeg に書き始めた時刻
    handed: Instant,
    /// 書いたときの区間 ([`CatchUp`])
    generation: Generation,
}

/// 受け取ってから解析するまでのフレーム。受信、書き込み、解析のスレッドで共有する。
#[derive(Default)]
struct FrameQueue {
    /// ffmpeg への書き込みを待っているフレームの数
    waiting: AtomicUsize,
    /// 書き込みのスレッドが、ffmpeg に書かずに捨てたフレームの数 (ログを出すたびに 0 に戻す)
    dropped_before_ffmpeg: AtomicUsize,
    /// ffmpeg に書いて、まだ出てきていないフレーム (書いた順)
    written: Mutex<VecDeque<WrittenFrame>>,
    /// 解析のスレッドが遅れて捨てた画像の区間に 1 を足したもの。0 ならない
    stale: AtomicU64,
}

impl FrameQueue {
    /// 解析のスレッドが、遅れた画像を捨てたことを書き込みのスレッドに知らせる。
    fn report_stale(&self, generation: Generation) {
        self.stale.fetch_max(generation.0 + 1, Ordering::Relaxed);
    }

    /// 知らされた区間を受け取って、消す。
    fn take_stale(&self) -> Option<Generation> {
        self.stale
            .swap(0, Ordering::Relaxed)
            .checked_sub(1)
            .map(Generation)
    }
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
        let layout = DecodeLayout::from_settings(&settings);
        let mut child = Command::new(&options.ffmpeg)
            .args(ffmpeg_args(&settings, options.hwaccel.as_deref()))
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
                .spawn(move || write_loop(camera_id, &input_rx, &mut stdin, &queue))?;
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
                        layout,
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

    /// 動画と解析画像の大きさが同じなら、設定 (範囲、視線の計算) だけ差し替える。違えば `false` を返す。
    pub fn update_settings(&self, settings: AnalysisSettings) -> bool {
        let mut current = self.settings.lock();
        if settings.width != self.width
            || settings.height != self.height
            || settings.video_width != current.video_width
            || settings.video_height != current.video_height
        {
            return false;
        }
        *current = settings;
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
            key_frame: frame.key_frame,
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

fn write_loop(
    camera_id: u32,
    input_rx: &Receiver<Input>,
    stdin: &mut impl Write,
    queue: &FrameQueue,
) {
    let mut catch_up = CatchUp::default();
    while let Ok(input) = input_rx.recv() {
        let stale = if input.frame.is_some() {
            queue.waiting.fetch_sub(1, Ordering::Relaxed);
            queue.take_stale()
        } else {
            None
        };
        match catch_up.prepare(&input, Instant::now(), stale) {
            WriteAction::Skip { started_with_delay } => {
                if input.frame.is_some() {
                    queue.dropped_before_ffmpeg.fetch_add(1, Ordering::Relaxed);
                }
                if let Some(delay) = started_with_delay {
                    warn!(
                        camera_id,
                        delay_ms = delay.as_millis(),
                        "video analysis is falling behind; skipping to the next key frame"
                    );
                }
                continue;
            }
            WriteAction::Write {
                config,
                resumed_after,
            } => {
                if let Some(config) = config
                    && stdin.write_all(&config).is_err()
                {
                    break;
                }
                if resumed_after > 0 {
                    info!(
                        camera_id,
                        dropped = resumed_after,
                        "resuming video at a key frame"
                    );
                }
            }
        }
        let Input { data, frame } = input;
        // 時刻は書く前に積む (ffmpeg の出力より先に積まれているように)
        if let Some(meta) = frame {
            queue.written.lock().push_back(WrittenFrame {
                meta,
                handed: Instant::now(),
                generation: catch_up.generation,
            });
        }
        if stdin.write_all(&data).and_then(|()| stdin.flush()).is_err() {
            break;
        }
    }
}

fn analyze_loop(
    camera_id: u32,
    stdout: &mut impl Read,
    width: usize,
    height: usize,
    layout: DecodeLayout,
    queue: &FrameQueue,
    settings: &Mutex<AnalysisSettings>,
    mut snapshot_dir: Option<PathBuf>,
    detection: super::analysis::DetectionOptions,
    out: &mpsc::Sender<(DateTime<Utc>, DetectionsPacket)>,
) {
    let mut analyzer = Analyzer::new(settings.lock().clone()).with_options(detection);
    let mut image = vec![0u8; width * height * 3];
    let mut nv12 = vec![0u8; layout.frame_bytes()];
    let mut stats = Stats::new();
    loop {
        let read_begin = Instant::now();
        if stdout.read_exact(&mut nv12).is_err() {
            break;
        }
        let decoded = Instant::now();
        let Some(WrittenFrame {
            meta,
            handed,
            generation,
        }) = queue.written.lock().pop_front()
        else {
            warn!(camera_id, "a decoded frame has no timestamp; skipping it");
            continue;
        };
        stats.add_input(
            handed - meta.received,
            decoded - handed,
            decoded - read_begin,
        );
        if decoded - meta.received > MAX_DELAY {
            // 背景の更新も飛ぶが、ffmpeg に残った古い画像を速く読み流すことを優先する。書き込みのスレッドにも
            // 知らせて、デコードしても捨てるだけのフレームを書かないようにする
            queue.report_stale(generation);
            stats.dropped_after_ffmpeg += 1;
            stats.log_if_due(camera_id, queue);
            continue;
        }
        let convert_begin = Instant::now();
        layout.convert(&nv12, &mut image);
        stats.converting += convert_begin.elapsed();
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
        stats.log_if_due(camera_id, queue);
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
    dropped_before_ffmpeg: usize,
    dropped_after_ffmpeg: usize,
    with_detections: u32,
    ignored: usize,
    analysis: Duration,
    converting: Duration,
    /// 受け取ってから解析し終えるまで (デコードの待ちを含む)。フレームごと (ms)
    delays_ms: Vec<i64>,
    /// 受け取ってから ffmpeg に書き始めるまで。フレームごと
    inputs: Vec<Duration>,
    /// ffmpeg に書き始めてから、NV12 を読み終えるまで。フレームごと。解析が遅れて ffmpeg の出力が
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
            dropped_before_ffmpeg: 0,
            dropped_after_ffmpeg: 0,
            with_detections: 0,
            ignored: 0,
            analysis: Duration::ZERO,
            converting: Duration::ZERO,
            delays_ms: Vec::new(),
            inputs: Vec::new(),
            decodes: Vec::new(),
            waiting: Duration::ZERO,
            sending: Duration::ZERO,
        }
    }

    fn log_if_due(&mut self, camera_id: u32, queue: &FrameQueue) {
        if self.started.elapsed() >= STATS_INTERVAL {
            self.dropped_before_ffmpeg = queue.dropped_before_ffmpeg.swap(0, Ordering::Relaxed);
            self.log(
                camera_id,
                queue.waiting.load(Ordering::Relaxed),
                queue.written.lock().len(),
            );
            *self = Self::new();
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
            dropped_before_ffmpeg = self.dropped_before_ffmpeg,
            dropped_after_ffmpeg = self.dropped_after_ffmpeg,
            frames_with_detections = self.with_detections,
            ignored_detections = self.ignored,
            analysis_ms = mean_ms(self.analysis),
            convert_ms = mean_ms(self.converting),
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

    fn frame(received: Instant, key_frame: bool) -> Input {
        Input {
            data: vec![1],
            frame: Some(FrameMeta {
                sensor_timestamp_ns: 0,
                timestamp_ms: 0,
                received_at: Utc::now(),
                received,
                key_frame,
            }),
        }
    }

    const WRITE: WriteAction = WriteAction::Write {
        config: None,
        resumed_after: 0,
    };
    const SKIP: WriteAction = WriteAction::Skip {
        started_with_delay: None,
    };

    fn config(data: u8) -> Input {
        Input {
            data: vec![data],
            frame: None,
        }
    }

    #[test]
    fn catch_up_resumes_only_at_a_fresh_key_frame() {
        let now = Instant::now();
        let late = MAX_DELAY + Duration::from_millis(1);
        let mut catch_up = CatchUp::default();
        assert_eq!(
            catch_up.prepare(&frame(now - late, false), now, None),
            WriteAction::Skip {
                started_with_delay: Some(late),
            }
        );
        assert_eq!(catch_up.prepare(&frame(now, false), now, None), SKIP);
        // 遅れたキーフレームからは再開しない
        assert_eq!(catch_up.prepare(&frame(now - late, true), now, None), SKIP);
        assert_eq!(
            catch_up.prepare(&frame(now, true), now, None),
            WriteAction::Write {
                config: None,
                resumed_after: 3
            }
        );
        assert_eq!(catch_up.generation, Generation(1));
        assert_eq!(catch_up.prepare(&frame(now, false), now, None), WRITE);
    }

    #[test]
    fn catch_up_rewrites_the_latest_config_before_resuming() {
        let now = Instant::now();
        let mut catch_up = CatchUp::default();
        assert_eq!(catch_up.prepare(&config(1), now, None), WRITE);
        catch_up.prepare(
            &frame(now - MAX_DELAY - Duration::from_millis(1), false),
            now,
            None,
        );
        for value in [2, 3] {
            assert_eq!(catch_up.prepare(&config(value), now, None), SKIP);
        }
        assert_eq!(
            catch_up.prepare(&frame(now, true), now, None),
            WriteAction::Write {
                config: Some(vec![3]),
                resumed_after: 1
            }
        );
    }

    #[test]
    fn catch_up_keeps_frames_within_the_delay_limit() {
        let now = Instant::now();
        let mut catch_up = CatchUp::default();
        for key_frame in [true, false, false, true] {
            assert_eq!(
                catch_up.prepare(&frame(now, key_frame), now + MAX_DELAY, None),
                WRITE
            );
        }
        assert_eq!(catch_up.generation, Generation(0));
    }

    #[test]
    fn catch_up_skips_when_the_current_generation_was_dropped_after_decoding() {
        let now = Instant::now();
        let mut catch_up = CatchUp::default();
        assert_eq!(
            catch_up.prepare(&frame(now, false), now, Some(Generation(0))),
            WriteAction::Skip {
                started_with_delay: Some(Duration::ZERO),
            }
        );
        assert_eq!(
            catch_up.prepare(&frame(now, true), now, None),
            WriteAction::Write {
                config: None,
                resumed_after: 1
            }
        );
        // 再開する前に書いた画像が捨てられても、新しい区間は捨てない
        assert_eq!(
            catch_up.prepare(&frame(now, false), now, Some(Generation(0))),
            WRITE
        );
    }

    #[test]
    fn catch_up_restarts_the_generation_at_a_fresh_key_frame_without_skipping() {
        let now = Instant::now();
        let mut catch_up = CatchUp::default();
        assert_eq!(
            catch_up.prepare(&frame(now, true), now, Some(Generation(0))),
            WRITE
        );
        assert_eq!(catch_up.generation, Generation(1));
    }

    #[test]
    fn frame_queue_reports_the_latest_stale_generation_once() {
        let queue = FrameQueue::default();
        assert_eq!(queue.take_stale(), None);
        queue.report_stale(Generation(0));
        queue.report_stale(Generation(2));
        queue.report_stale(Generation(1));
        assert_eq!(queue.take_stale(), Some(Generation(2)));
        assert_eq!(queue.take_stale(), None);
    }

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
