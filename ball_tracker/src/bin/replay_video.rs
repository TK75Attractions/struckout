//! 録画した動画 (ball_tracker の `--video-record-dir` が書いた `.vsp`) を解析し直し、追跡をやり直して、
//! 着弾を出す。解析の設定やカメラの位置を変えて何度でも回せる。
//!
//! ```text
//! replay_video <記録フォルダー> --poses poses.json --out <出力フォルダー> [--from 10:40:00 --to 10:55:00]
//! ```
//!
//! - 記録フォルダーには、`video/` (`.vsp`) と、その回の `frames_*` (`-j` の記録) を置く
//!   (`scripts/run_pc1.py --record` がそうする)。
//! - 撮影時刻は、その回の `frames_*` から、センサー時刻と「この PC の時計に直した時刻」の対応を取って求める。
//!   その場の時刻合わせと同じ結果になる。
//! - 解析は、その場の PC 解析 (`ball_tracker::video`) と同じ処理。設定はカメラが送ってきた値で、
//!   `--min-color-shift` などで上書きできる。
//! - 追跡は、各フレームが撮影から `--latency-ms` 後に届いたものとして進める (その場の遅れは使わない)。
//!
//! 出力:
//! - `collisions.csv`: 着弾ごとの報告時刻、予測した着弾時刻、位置 (y, z)、視線の本数、残差、時刻のずれ
//! - `events.jsonl`: 追跡の記録 (`-j` の `events_*` と同じ形)
//! - `detections_camera<番号>.csv`: 検出の一覧 (`--detections` を付けたとき)
//!
//! 動画の解析には時間がかかる (3 分の録画で数分)。追跡の設定やカメラの位置だけを変えて回すときは、
//! 1 回目に `--save-detections` で解析の結果を保存し、2 回目からは `--load-detections` で読み込むと、
//! 動画を解析せずに追跡だけをやり直す (数秒)。
//!
//! カメラの位置 (`--poses`) の形:
//! ```json
//! { "0": { "position_m": [0.075, 1.08, 1.258], "rotation_deg": [131.8, -1.8, -150.0] }, "1": { ... } }
//! ```

use std::{
    collections::{BTreeMap, HashMap},
    fs::File,
    io::{BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, mpsc},
    thread,
    time::Instant,
};

use anyhow::{Context, Result, bail};
use ball_tracker::{
    CameraLocationStore,
    clock_sync::AutoTimeTrim,
    detection_input::CameraFrame,
    tracking::{EventLogger, TrackId, TrackRunner, TrackingEventBodyDto, TrackingEventsDto},
    types::CameraId,
    video::{
        analysis::{AnalysisSettings, Analyzer},
        recorder::{RecordingReader, recordings_in},
    },
};
use chrono::{DateTime, Local, NaiveTime, TimeDelta, TimeZone, Utc};
use clap::Parser;
use serde::{Deserialize, Serialize};
use struckout_proto::{CameraPose, DetectionsPacket, VideoFrame, video_stream_packet};

#[derive(Parser)]
struct Cli {
    /// 記録フォルダー (`video/` と `frames_*` があるところ)
    dir: PathBuf,
    /// カメラの位置と向き (JSON)
    #[arg(long)]
    poses: PathBuf,
    /// 出力フォルダー
    #[arg(long)]
    out: PathBuf,
    /// 時刻の対応に使う `frames_*`。省略すると記録フォルダーの最も新しいもの
    #[arg(long)]
    frames: Option<PathBuf>,
    /// 解析し始める時刻 (この PC の時計、その日の HH:MM:SS)。省略すると録画の最初から
    #[arg(long)]
    from: Option<NaiveTime>,
    /// 解析を終える時刻。省略すると録画の最後まで
    #[arg(long)]
    to: Option<NaiveTime>,
    #[arg(long, default_value = "ffmpeg")]
    ffmpeg: String,
    /// デコードに使う GPU の方式。ball_tracker の `--video-hwaccel` と同じ (auto、none、d3d11va など)
    #[arg(long, default_value = "none")]
    hwaccel: String,
    /// 視線を作る検出の位置。ball_tracker の `--detection-center` と同じ (box、centroid)
    #[arg(long, default_value = "box")]
    detection_center: ball_tracker::video::analysis::DetectionCenter,
    /// 画像の端に接した検出の扱い。ball_tracker の `--edge-detections` と同じ (keep、drop、extrapolate)
    #[arg(long, default_value = "keep")]
    edge_detections: ball_tracker::video::analysis::EdgeDetections,
    /// 画像の上端と下端の露光開始の時刻の差 (ms)。KYV45 の 120fps は 8.333
    #[arg(long, default_value_t = 8.333)]
    row_skew_ms: f64,
    /// 右のカメラ (位置の y が大きいほう) の撮影時刻に足す補正 (ms)
    #[arg(long, default_value_t = 0.0, allow_negative_numbers = true)]
    time_trim_ms: f64,
    /// 撮影から追跡に届くまでの時間 (ms)。追跡は届いた時刻を「今」として進める
    #[arg(long, default_value_t = 100.0)]
    latency_ms: f64,
    /// 着弾を、予測した着弾時刻の何 ms 後に報告するか (負なら前)。ball_tracker の `--report-delay-ms` と同じ
    #[arg(long, default_value_t = -30.0, allow_negative_numbers = true)]
    report_delay_ms: f64,
    /// 右のカメラの時刻の補正を自動で決めて、`--time-trim-ms` に足す。ball_tracker の `--auto-time-trim` と同じ
    #[arg(long)]
    auto_time_trim: bool,
    /// 解析の設定の上書き (省略するとカメラが送ってきた値)
    #[arg(long)]
    min_color_shift: Option<f64>,
    #[arg(long)]
    binary_threshold: Option<f64>,
    #[arg(long)]
    min_contour_area: Option<f64>,
    #[arg(long)]
    persistence: Option<u32>,
    #[arg(long)]
    accumulate_weight: Option<f64>,
    /// 検出の一覧も書く
    #[arg(long)]
    detections: bool,
    /// 解析の結果 (検出のあったフレーム) をこのファイルに保存する (JSON Lines)
    #[arg(long)]
    save_detections: Option<PathBuf>,
    /// 動画を解析せず、`--save-detections` で保存した結果を読み込む。解析の設定の上書きは効かない
    #[arg(long)]
    load_detections: Option<PathBuf>,
    /// カメラの焦点距離を、カメラが送ってきた値の何倍とみなすか (`<カメラの番号>=<倍率>`、何度でも書ける)。
    /// 視線をその比で伸び縮みさせる (動画を解析し直したのと同じ結果になる)
    #[arg(long, value_parser = parse_camera_value)]
    focal_scale: Vec<(u32, f64)>,
}

fn parse_camera_value(text: &str) -> Result<(u32, f64), String> {
    let (id, value) = text.split_once('=').ok_or("expected <camera id>=<value>")?;
    Ok((
        id.parse().map_err(|e| format!("{e}"))?,
        value.parse().map_err(|e| format!("{e}"))?,
    ))
}

/// 記録フォルダーの `.vsp` から、カメラごとの最初の設定 (端末の座標 → カメラの座標の回転) を読む。
fn device_to_camera(dir: &Path) -> Result<HashMap<u32, [f64; 9]>> {
    let mut out = HashMap::new();
    for path in recordings_in(&dir.join("video"))? {
        for item in RecordingReader::open(&path)? {
            if let Some(video_stream_packet::Data::Config(config)) = item?.1.data {
                let settings =
                    AnalysisSettings::from_config(&config).map_err(anyhow::Error::msg)?;
                out.insert(settings.camera_id, settings.device_to_camera);
                break;
            }
        }
    }
    Ok(out)
}

/// 焦点距離を `scale` 倍とみなしたときの視線。視線は R^T (x, y, 1) なので、カメラの座標に戻して x, y を割る。
fn rescale_ray(r: &[f64; 9], lay: [f64; 3], scale: f64) -> [f64; 3] {
    let n = [
        r[0] * lay[0] + r[1] * lay[1] + r[2] * lay[2],
        r[3] * lay[0] + r[4] * lay[1] + r[5] * lay[2],
        r[6] * lay[0] + r[7] * lay[1] + r[8] * lay[2],
    ];
    let n = [n[0] / scale, n[1] / scale, n[2]];
    [
        r[0] * n[0] + r[3] * n[1] + r[6] * n[2],
        r[1] * n[0] + r[4] * n[1] + r[7] * n[2],
        r[2] * n[0] + r[5] * n[1] + r[8] * n[2],
    ]
}

/// 範囲の前から解析し始める時間。背景を作るのに要る
const WARMUP: TimeDelta = TimeDelta::seconds(2);
/// 追跡の時計を進める間隔 (その場の `COLLISION_POLL_INTERVAL` と同じ)
const POLL_STEP: TimeDelta = TimeDelta::milliseconds(5);

#[derive(Deserialize)]
struct PoseEntry {
    position_m: [f64; 3],
    rotation_deg: [f64; 3],
}

#[derive(Deserialize)]
struct FrameLine {
    time: DateTime<Utc>,
    packet: FramePacket,
}

#[derive(Deserialize)]
struct FramePacket {
    camera_id: u32,
    #[serde(default)]
    sensor_timestamp_ns: i64,
}

/// カメラごとの「センサー時刻 → この PC の時計」の対応。その場の時刻合わせの結果 (`frames_*`) から作る。
struct ClockMap(HashMap<u32, Vec<(i64, i64)>>);

impl ClockMap {
    fn load(path: &Path) -> Result<Self> {
        let reader = BufReader::new(
            File::open(path).with_context(|| format!("cannot open {}", path.display()))?,
        );
        let mut map: HashMap<u32, Vec<(i64, i64)>> = HashMap::new();
        for line in serde_json::Deserializer::from_reader(reader).into_iter::<FrameLine>() {
            let line = line.with_context(|| format!("cannot read {}", path.display()))?;
            if line.packet.sensor_timestamp_ns == 0 {
                continue;
            }
            let time_ns = line
                .time
                .timestamp_nanos_opt()
                .context("time out of range")?;
            map.entry(line.packet.camera_id).or_default().push((
                line.packet.sensor_timestamp_ns,
                time_ns - line.packet.sensor_timestamp_ns,
            ));
        }
        for samples in map.values_mut() {
            samples.sort_unstable();
        }
        Ok(Self(map))
    }

    /// センサー時刻に最も近いフレームの対応を使う (時刻合わせが途中で直した分も、その場と同じになる)。
    fn capture_time(&self, camera_id: u32, sensor_ns: i64) -> Option<DateTime<Utc>> {
        let samples = self.0.get(&camera_id)?;
        let i = samples.partition_point(|(s, _)| *s < sensor_ns);
        let nearest = [i.checked_sub(1), Some(i)]
            .into_iter()
            .flatten()
            .filter_map(|j| samples.get(j))
            .min_by_key(|(s, _)| (s - sensor_ns).abs())?;
        Some(DateTime::from_timestamp_nanos(sensor_ns + nearest.1))
    }
}

struct Overrides {
    min_color_shift: Option<f64>,
    binary_threshold: Option<f64>,
    min_contour_area: Option<f64>,
    persistence: Option<u32>,
    accumulate_weight: Option<f64>,
}

impl Overrides {
    fn apply(&self, settings: &mut AnalysisSettings) {
        let t = &mut settings.tracker;
        if let Some(v) = self.min_color_shift {
            t.min_color_shift = v;
        }
        if let Some(v) = self.binary_threshold {
            t.binary_threshold = v;
        }
        if let Some(v) = self.min_contour_area {
            t.min_contour_area = v;
        }
        if let Some(v) = self.persistence {
            t.foreground_persistence_frames = v;
        }
        if let Some(v) = self.accumulate_weight {
            t.accumulate_weight = v;
        }
    }
}

/// 解析した 1 フレーム (検出のあったもの)。
#[derive(Serialize, Deserialize)]
struct Detected {
    camera_id: u32,
    captured_at: DateTime<Utc>,
    packet: DetectionsPacket,
}

enum Meta {
    Settings(Box<AnalysisSettings>),
    Frame {
        sensor_ns: i64,
        timestamp_ms: i64,
        captured_at: DateTime<Utc>,
    },
}

struct CameraStats {
    camera_id: u32,
    written: usize,
    decoded: usize,
    with_detections: usize,
    settings: Option<AnalysisSettings>,
}

/// `.vsp` 1 本を ffmpeg でデコードして解析する。
fn analyze_recording(
    path: &Path,
    clock: &ClockMap,
    range: (Option<DateTime<Utc>>, Option<DateTime<Utc>>),
    overrides: &Overrides,
    ffmpeg: &str,
    hwaccel: Option<&str>,
    detection: ball_tracker::video::analysis::DetectionOptions,
) -> Result<(Vec<Detected>, CameraStats)> {
    let mut packets = RecordingReader::open(path)?;
    // 最初の設定を探す (解析画像の大きさが決まらないと ffmpeg を起動できない)
    let mut pending_frames: Vec<VideoFrame> = Vec::new();
    let mut settings = loop {
        let Some(item) = packets.next() else {
            bail!("{}: no VideoStreamConfig", path.display());
        };
        match item?.1.data {
            Some(video_stream_packet::Data::Config(config)) => {
                let mut settings =
                    AnalysisSettings::from_config(&config).map_err(anyhow::Error::msg)?;
                overrides.apply(&mut settings);
                break settings;
            }
            Some(video_stream_packet::Data::Frame(frame)) => pending_frames.push(frame),
            None => {}
        }
    };
    let camera_id = settings.camera_id;
    let (width, height) = (settings.width, settings.height);
    let layout = ball_tracker::video::decode::DecodeLayout::from_settings(&settings);
    let mut child = Command::new(ffmpeg)
        .args(ball_tracker::video::decode::ffmpeg_args(&settings, hwaccel))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .with_context(|| format!("cannot start ffmpeg ({ffmpeg})"))?;
    let mut stdin = child.stdin.take().expect("piped");
    let mut stdout = child.stdout.take().expect("piped");
    let (meta_tx, meta_rx) = mpsc::channel::<Meta>();

    let first_settings = settings.clone();
    let reader = thread::spawn(move || {
        let mut analyzer = Analyzer::new(first_settings).with_options(detection);
        let mut image = vec![0u8; width * height * 3];
        let mut nv12 = vec![0u8; layout.frame_bytes()];
        let mut out = Vec::new();
        let mut decoded = 0;
        while stdout.read_exact(&mut nv12).is_ok() {
            let (sensor_ns, timestamp_ms, captured_at) = loop {
                match meta_rx.recv() {
                    Ok(Meta::Settings(settings)) => analyzer.update(*settings),
                    Ok(Meta::Frame {
                        sensor_ns,
                        timestamp_ms,
                        captured_at,
                    }) => break (sensor_ns, timestamp_ms, captured_at),
                    Err(_) => return (out, decoded),
                }
            };
            decoded += 1;
            layout.convert(&nv12, &mut image);
            if let Some(packet) = analyzer.analyze(&image, sensor_ns, timestamp_ms).packet {
                out.push(Detected {
                    camera_id,
                    captured_at,
                    packet,
                });
            }
        }
        (out, decoded)
    });

    let (from, to) = range;
    let start_at = from.map(|t| t - WARMUP);
    let mut codec_config: Option<Vec<u8>> = None;
    let mut started = false;
    let mut written = 0;
    let mut unmapped = 0;
    let frames = pending_frames
        .into_iter()
        .map(|f| Ok(Some(f)))
        .chain(packets.map(|item| {
            item.map(|(_, packet)| match packet.data {
            Some(video_stream_packet::Data::Frame(frame)) => Some(frame),
            Some(video_stream_packet::Data::Config(config)) => {
                match AnalysisSettings::from_config(&config) {
                    Ok(mut next) => {
                        overrides.apply(&mut next);
                        if next.width == width && next.height == height
                            && next.video_width == settings.video_width
                            && next.video_height == settings.video_height {
                            settings = next.clone();
                            let _ = meta_tx.send(Meta::Settings(Box::new(next)));
                        } else {
                            eprintln!(
                                "camera {camera_id}: video or analysis size changed; ignored the new config"
                            );
                        }
                    }
                    Err(e) => eprintln!("camera {camera_id}: invalid config: {e}"),
                }
                None
            }
            None => None,
        })
        }));
    for frame in frames {
        let Some(frame) = frame? else { continue };
        if frame.codec_config {
            if started {
                stdin.write_all(&frame.data)?;
            }
            codec_config = Some(frame.data);
            continue;
        }
        let Some(captured_at) = clock.capture_time(camera_id, frame.sensor_timestamp_ns) else {
            unmapped += 1;
            continue;
        };
        if to.is_some_and(|to| captured_at > to) {
            break;
        }
        if !started {
            if !frame.key_frame || start_at.is_some_and(|t| captured_at < t) {
                continue;
            }
            if let Some(config) = &codec_config {
                stdin.write_all(config)?;
            }
            started = true;
        }
        // 時刻は書く前に送る (ffmpeg の出力より先に届いているように)
        let _ = meta_tx.send(Meta::Frame {
            sensor_ns: frame.sensor_timestamp_ns,
            timestamp_ms: frame.timestamp,
            captured_at,
        });
        stdin.write_all(&frame.data)?;
        written += 1;
    }
    drop(stdin);
    drop(meta_tx);
    let (detected, decoded) = reader.join().expect("analysis thread panicked");
    child.wait()?;
    if unmapped > 0 {
        eprintln!(
            "camera {camera_id}: {unmapped} frames had no time mapping in frames_* and were skipped"
        );
    }
    let with_detections = detected.len();
    Ok((
        detected,
        CameraStats {
            camera_id,
            written,
            decoded,
            with_detections,
            settings: Some(settings),
        },
    ))
}

/// 追跡にフレームを渡したときの記録 (フレームは書かない)。
struct NoFrameLog;

impl EventLogger for NoFrameLog {
    fn push_events(&mut self, _events: TrackingEventsDto) {}
    fn push_frame(&mut self, _frame: &CameraFrame) {}
}

#[derive(Default)]
struct CollisionRow {
    reported_at: Option<DateTime<Utc>>,
    impact_at: Option<DateTime<Utc>>,
    y: f64,
    z: f64,
    rays: BTreeMap<u32, usize>,
    residual_rms: Option<f64>,
    best_time_shift_ms: Option<i64>,
    best_residual_rms: Option<f64>,
}

/// 記録フォルダーの `.vsp` をすべて (カメラごとに並列で) 解析する。
fn analyze_all(
    cli: &Cli,
    clock: &ClockMap,
    range: (Option<DateTime<Utc>>, Option<DateTime<Utc>>),
    overrides: &Overrides,
) -> Result<Vec<Detected>> {
    let recordings = recordings_in(&cli.dir.join("video"))?;
    if recordings.is_empty() {
        bail!("no .vsp in {}", cli.dir.join("video").display());
    }
    let mut detected = Vec::new();
    let hwaccel = ball_tracker::video::hwaccel::select(&cli.ffmpeg, &cli.hwaccel);
    if let Some(name) = &hwaccel {
        eprintln!("decoding on the GPU ({name})");
    }
    let results: Vec<Result<(Vec<Detected>, CameraStats)>> = thread::scope(|scope| {
        let handles: Vec<_> = recordings
            .iter()
            .map(|path| {
                let ffmpeg = cli.ffmpeg.as_str();
                let hwaccel = hwaccel.as_deref();
                let detection = ball_tracker::video::analysis::DetectionOptions {
                    center: cli.detection_center,
                    edge: cli.edge_detections,
                };
                scope.spawn(move || {
                    analyze_recording(path, clock, range, overrides, ffmpeg, hwaccel, detection)
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("camera thread panicked"))
            .collect()
    });
    for (path, result) in recordings.iter().zip(results) {
        let (frames, stats) =
            result.with_context(|| format!("failed to analyze {}", path.display()))?;
        let tracker = stats.settings.as_ref().map(|s| s.tracker);
        eprintln!(
            "camera {}: {} -> written {} decoded {} with detections {} (tracker {:?})",
            stats.camera_id,
            path.file_name().unwrap_or_default().to_string_lossy(),
            stats.written,
            stats.decoded,
            stats.with_detections,
            tracker
        );
        if stats.written != stats.decoded {
            eprintln!(
                "  warning: ffmpeg returned a different number of frames; timestamps may be shifted"
            );
        }
        detected.extend(frames);
    }
    Ok(detected)
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let begin = Instant::now();
    std::fs::create_dir_all(&cli.out)?;

    // カメラの位置
    let poses: HashMap<String, PoseEntry> = serde_json::from_reader(BufReader::new(
        File::open(&cli.poses).with_context(|| format!("cannot open {}", cli.poses.display()))?,
    ))?;
    let locs = Arc::new(CameraLocationStore::new());
    let mut rightmost: Option<(u32, f64)> = None;
    for (id, pose) in &poses {
        let id: u32 = id
            .parse()
            .with_context(|| format!("camera id must be a number: {id}"))?;
        locs.insert(
            CameraId::new(id),
            CameraPose {
                x: pose.position_m[0],
                y: pose.position_m[1],
                z: pose.position_m[2],
                rotation_x_degrees: pose.rotation_deg[0],
                rotation_y_degrees: pose.rotation_deg[1],
                rotation_z_degrees: pose.rotation_deg[2],
            },
        );
        if rightmost.is_none_or(|(_, y)| pose.position_m[1] > y) {
            rightmost = Some((id, pose.position_m[1]));
        }
    }

    // 時刻の対応
    let frames_path = match &cli.frames {
        Some(path) => path.clone(),
        None => std::fs::read_dir(&cli.dir)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("frames_"))
            })
            .max_by_key(|p| p.metadata().and_then(|m| m.modified()).ok())
            .context("no frames_* in the folder; pass --frames")?,
    };
    let clock = ClockMap::load(&frames_path)?;
    eprintln!(
        "time mapping: {} ({} cameras)",
        frames_path.display(),
        clock.0.len()
    );

    // 解析する範囲 (この PC の時計の、その日の時刻)
    let day = clock
        .0
        .values()
        .flat_map(|v| v.first())
        .map(|(s, o)| {
            DateTime::from_timestamp_nanos(s + o)
                .with_timezone(&Local)
                .date_naive()
        })
        .next()
        .context("frames_* has no frames with sensor timestamps")?;
    let to_utc = |t: Option<NaiveTime>| -> Result<Option<DateTime<Utc>>> {
        t.map(|t| {
            Local
                .from_local_datetime(&day.and_time(t))
                .single()
                .map(|t| t.with_timezone(&Utc))
                .context("ambiguous local time")
        })
        .transpose()
    };
    let range = (to_utc(cli.from)?, to_utc(cli.to)?);

    // カメラごとに解析する (並列)
    let overrides = Overrides {
        min_color_shift: cli.min_color_shift,
        binary_threshold: cli.binary_threshold,
        min_contour_area: cli.min_contour_area,
        persistence: cli.persistence,
        accumulate_weight: cli.accumulate_weight,
    };
    let mut detected: Vec<Detected> = Vec::new();
    if let Some(path) = &cli.load_detections {
        let reader = BufReader::new(
            File::open(path).with_context(|| format!("cannot open {}", path.display()))?,
        );
        for item in serde_json::Deserializer::from_reader(reader).into_iter::<Detected>() {
            let item = item?;
            let inside = range.0.is_none_or(|from| item.captured_at >= from)
                && range.1.is_none_or(|to| item.captured_at <= to);
            if inside {
                detected.push(item);
            }
        }
        eprintln!(
            "loaded {} frames with detections from {}",
            detected.len(),
            path.display()
        );
    } else {
        detected = analyze_all(&cli, &clock, range, &overrides)?;
    }
    if !cli.focal_scale.is_empty() {
        let rotations = device_to_camera(&cli.dir)?;
        let scales: HashMap<u32, f64> = cli.focal_scale.iter().copied().collect();
        for d in &mut detected {
            let (Some(scale), Some(r)) = (scales.get(&d.camera_id), rotations.get(&d.camera_id))
            else {
                continue;
            };
            for det in &mut d.packet.detections {
                let [x, y, z] = rescale_ray(r, [det.lay_x, det.lay_y, det.lay_z], *scale);
                (det.lay_x, det.lay_y, det.lay_z) = (x, y, z);
            }
        }
    }
    if let Some(path) = &cli.save_detections {
        let mut w = BufWriter::new(File::create(path)?);
        for d in &detected {
            serde_json::to_writer(&mut w, d)?;
            writeln!(w)?;
        }
        w.flush()?;
    }

    if cli.detections {
        let mut writers: HashMap<u32, BufWriter<File>> = HashMap::new();
        for d in &detected {
            let w = match writers.entry(d.camera_id) {
                std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
                std::collections::hash_map::Entry::Vacant(e) => {
                    let mut w = BufWriter::new(File::create(
                        cli.out
                            .join(format!("detections_camera{}.csv", d.camera_id)),
                    )?);
                    writeln!(
                        w,
                        "captured_at,sensor_timestamp_ns,image_x,image_y,bbox_width,bbox_height"
                    )?;
                    e.insert(w)
                }
            };
            for det in &d.packet.detections {
                writeln!(
                    w,
                    "{},{},{:.5},{:.5},{},{}",
                    d.captured_at.with_timezone(&Local).format("%H:%M:%S%.3f"),
                    d.packet.sensor_timestamp_ns,
                    det.image_x,
                    det.image_y,
                    det.bbox_width,
                    det.bbox_height
                )?;
            }
        }
    }

    // 追跡をやり直す
    let trim = TimeDelta::nanoseconds((cli.time_trim_ms * 1e6).round() as i64);
    let latency = TimeDelta::nanoseconds((cli.latency_ms * 1e6).round() as i64);
    let row_skew_ns = (cli.row_skew_ms * 1e6).round() as i64;
    // 補正は追跡に渡す時点でかける (自動の補正は、それまでの投球で決まった値をその場と同じく使う)
    let mut frames: Vec<CameraFrame> = detected
        .into_iter()
        .map(|d| CameraFrame {
            time: d.captured_at,
            row_skew_ns,
            received_at: d.captured_at + latency,
            packet: d.packet,
        })
        .collect();
    frames.sort_by_key(|f| f.received_at);
    let right_camera = rightmost.map(|(id, _)| id);
    let auto_trim = Arc::new(AutoTimeTrim::new(cli.auto_time_trim));

    let mut runner = TrackRunner::new(locs, NoFrameLog)
        .with_report_delay(TimeDelta::nanoseconds(
            (cli.report_delay_ms * 1e6).round() as i64
        ))
        .with_auto_trim(auto_trim.clone());
    let mut events_out = BufWriter::new(File::create(cli.out.join("events.jsonl"))?);
    let mut rows: Vec<(TrackId, CollisionRow)> = Vec::new();
    let mut predicted: HashMap<TrackId, DateTime<Utc>> = HashMap::new();
    let mut handle = |dto: TrackingEventsDto,
                      rows: &mut Vec<(TrackId, CollisionRow)>|
     -> Result<()> {
        if dto.events.is_empty() {
            return Ok(());
        }
        for event in &dto.events {
            match event {
                TrackingEventBodyDto::NewTrack(s) | TrackingEventBodyDto::UpdateTrack(s) => {
                    if let Some(p) = &s.predicted_collision {
                        predicted.insert(s.track_id, p.time);
                    }
                }
                TrackingEventBodyDto::Collision(points) => {
                    for (id, point) in points {
                        rows.push((
                            *id,
                            CollisionRow {
                                reported_at: Some(dto.timestamp),
                                impact_at: predicted.get(id).copied(),
                                y: point.y,
                                z: point.z,
                                ..Default::default()
                            },
                        ));
                    }
                }
                TrackingEventBodyDto::ThrowDiagnostics(d) => {
                    if let Some((_, row)) = rows.iter_mut().rev().find(|(id, _)| *id == d.track_id)
                    {
                        row.rays = d.rays.clone();
                        row.residual_rms = Some(d.residual_rms);
                        row.best_time_shift_ms = Some(d.best_time_shift_ms);
                        row.best_residual_rms = Some(d.best_residual_rms);
                    }
                }
                _ => {}
            }
        }
        serde_json::to_writer(&mut events_out, &dto)?;
        writeln!(events_out)?;
        Ok(())
    };
    let mut clock_now = frames.first().map(|f| f.received_at);
    let last = frames.last().map(|f| f.received_at + TimeDelta::seconds(1));
    for mut frame in frames {
        if right_camera == Some(frame.packet.camera_id) {
            frame.time += trim + auto_trim.current(frame.packet.camera_id);
        }
        // フレームが届くまでのあいだも、その場と同じく時計で報告を進める
        while let Some(now) = clock_now.filter(|now| *now + POLL_STEP < frame.received_at) {
            let now = now + POLL_STEP;
            handle(runner.poll_collisions(now).1, &mut rows)?;
            clock_now = Some(now);
        }
        let now = frame.received_at;
        handle(runner.update_frame(frame, now).1, &mut rows)?;
        clock_now = Some(now);
    }
    if let (Some(mut now), Some(last)) = (clock_now, last) {
        while now < last {
            now += POLL_STEP;
            handle(runner.poll_collisions(now).1, &mut rows)?;
        }
    }
    drop(handle);
    events_out.flush()?;

    let cameras: Vec<u32> = {
        let mut ids: Vec<u32> = poses.keys().filter_map(|k| k.parse().ok()).collect();
        ids.sort_unstable();
        ids
    };
    let mut csv = BufWriter::new(File::create(cli.out.join("collisions.csv"))?);
    write!(csv, "reported_at,impact_at,y_m,z_m")?;
    for id in &cameras {
        write!(csv, ",rays_camera{id}")?;
    }
    writeln!(
        csv,
        ",residual_rms_cm,best_time_shift_ms,best_residual_rms_cm"
    )?;
    let local = |t: Option<DateTime<Utc>>| {
        t.map(|t| t.with_timezone(&Local).format("%H:%M:%S%.3f").to_string())
            .unwrap_or_default()
    };
    let opt = |v: Option<f64>| v.map(|v| format!("{:.2}", v * 100.0)).unwrap_or_default();
    for (_, row) in &rows {
        write!(
            csv,
            "{},{},{:.4},{:.4}",
            local(row.reported_at),
            local(row.impact_at),
            row.y,
            row.z
        )?;
        for id in &cameras {
            write!(csv, ",{}", row.rays.get(id).copied().unwrap_or_default())?;
        }
        writeln!(
            csv,
            ",{},{},{}",
            opt(row.residual_rms),
            row.best_time_shift_ms
                .map(|v| v.to_string())
                .unwrap_or_default(),
            opt(row.best_residual_rms)
        )?;
    }
    csv.flush()?;
    let (auto_ms, auto_throws) = auto_trim.summary();
    eprintln!(
        "automatic time trim: {auto_ms:+.1} ms from {auto_throws} throws ({})",
        if cli.auto_time_trim {
            "applied"
        } else {
            "not applied"
        }
    );
    eprintln!(
        "{} collisions -> {} ({:.1} s)",
        rows.len(),
        cli.out.join("collisions.csv").display(),
        begin.elapsed().as_secs_f64()
    );
    Ok(())
}
