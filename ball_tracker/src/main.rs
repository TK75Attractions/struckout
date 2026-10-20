use std::{ffi::OsString, path::PathBuf, sync::Arc};

#[cfg(feature = "input-sqlite")]
use ball_tracker::detection_input::SqliteDetectionInput;
use ball_tracker::{
    Application, CameraLocationStore,
    camera_config::CameraConfig,
    clock_sync::{AutoTimeTrim, ClockSync, ClockSyncOptions},
    collision_output::{CollisionOutput, CsvCollisionOutput, NetworkCollisionOutput},
    detection_input::{CameraFrame, DetectionInput, NetworkDetectionInput},
    tracking::{
        EventLogger, FmtEventLogger, JsonEventLogger, SentryEventLogger, TrackingEventsDto,
    },
    types::{CameraId, CollisionPoint3D},
    video::{self, VideoOptions},
};
use chrono::{Local, TimeDelta};
use clap::{Parser, ValueEnum};
use tokio::sync::mpsc;
use tracing_subscriber::{layer::SubscriberExt as _, util::SubscriberInitExt as _};

use crate::config_file::ConfigLocation;

mod config_file;

#[cfg(feature = "sentry")]
const SENTRY_DSN: &str = "https://2b9af00fb25f2faa692253f8ed95c43e@o4512229597315072.ingest.us.sentry.io/4512229769871360";

#[derive(Parser)]
#[command(version = version(), args_override_self = true)]
struct Cli {
    /// 設定ファイル (YAML) の場所。既定は <設定のフォルダー>/struckout/ball-tracker/config.yaml。
    /// キーは長いオプション名で、同じ引数はコマンドラインのほうが優先される (README を参照)
    #[arg(long = "config")]
    #[expect(dead_code, reason = "parse_cli() reads it before parsing Cli")]
    config: Option<PathBuf>,
    #[arg(value_enum, long = "input", help = "detectionをどこから受け取るか")]
    detection_input: DetectionInputKind,
    #[arg(value_enum, long = "output", help = "collisionをどこに送信するか")]
    collision_output: CollisionOutputKind,
    /// JSONに追跡結果を出力する
    #[arg(short = 'j', long = "json")]
    output_to_json: bool,
    #[arg(short = 'c', long = "csv_path")]
    csv_path: Option<PathBuf>,
    /// 右のカメラ (位置の y が大きいほう) の時刻に足す補正 (ms)。estimate_time_offset.py で求める
    #[arg(
        long = "time-trim-ms",
        default_value_t = 0.0,
        allow_negative_numbers = true
    )]
    time_trim_ms: f64,
    /// 画像の上端の行と下端の行の露光開始の時刻の差 (ms)。指定すると、カメラが知らせた値の代わりに使う
    #[arg(long = "rolling-shutter-skew-ms", allow_negative_numbers = true)]
    rolling_shutter_skew_ms: Option<f64>,
    /// カメラから動画を受け取って解析するときに使う ffmpeg (PATH にあれば名前だけでよい)
    #[arg(long = "ffmpeg", default_value = "ffmpeg")]
    ffmpeg: String,
    /// カメラから動画を受け取らない (5052 番で待ち受けない)。カメラは自分で解析する
    #[arg(long = "no-video-analysis")]
    no_video_analysis: bool,
    /// 指定すると、カメラごとに PC で解析し始めた最初の画像をこのフォルダーに BMP で保存する (向きの確認用)
    #[arg(long = "video-snapshot-dir")]
    video_snapshot_dir: Option<PathBuf>,
    /// 指定すると、カメラから受け取った動画をこのフォルダーに保存する (あとで解析し直すため)
    #[arg(long = "video-record-dir")]
    video_record_dir: Option<PathBuf>,
    /// 着弾を、予測した着弾時刻の何 ms 後に報告するか (負なら前)。遅らせるほど着弾位置が正確になるが、
    /// 的への描画がそのぶん遅れる
    #[arg(long = "report-delay-ms", default_value_t = -30.0, allow_negative_numbers = true)]
    report_delay_ms: f64,
    /// 右のカメラの時刻の補正を、報告した投球から自動で決めて、--time-trim-ms に足す。付けないときは、
    /// 決めた値をログに出すだけ。カメラの位置と向きが合っているときだけ使うこと
    #[arg(long = "auto-time-trim")]
    auto_time_trim: bool,
    /// カメラから受け取った動画のデコードに使う GPU の方式 (ffmpeg の -hwaccel)。auto はその PC で使えるものを
    /// 選ぶ (Windows は d3d11va、cuda、qsv、dxva2 の順)。none は CPU。d3d11va などの名前も指定できる
    #[arg(long = "video-hwaccel", default_value = "auto")]
    video_hwaccel: String,
    /// PC で解析するとき、視線を作る検出の位置。box は枠の中心 (カメラアプリと同じ)、centroid は前景の画素の重心
    #[arg(long = "detection-center", default_value = "box")]
    detection_center: video::analysis::DetectionCenter,
    /// PC で解析するとき、画像の端に接した検出 (ボールが欠けている) の扱い。keep はそのまま (カメラアプリと同じ)、
    /// drop は捨てる、extrapolate は欠けていない向きの幅から中心を推定する
    #[arg(long = "edge-detections", default_value = "keep")]
    edge_detections: video::analysis::EdgeDetections,
    /// カメラ用スマホごとの設定 (scripts/cameras.json)。書いた端末は、つないだ順によらず書いた順の camera_id
    /// (0, 1, ...) になり、ここに書いた位置と向きを使う。位置と向き、焦点距離、検出の範囲はスマホにも送る
    #[arg(long = "cameras")]
    cameras: Option<PathBuf>,
}

fn version() -> String {
    if let Some(version) = option_env!("ST_VERSION") {
        format!(
            "{} ({})",
            version,
            option_env!("ST_COMMIT_SHORT_HASH").unwrap()
        )
    } else {
        format!("no version")
    }
}

fn millis(value: f64) -> TimeDelta {
    TimeDelta::nanoseconds((value * 1e6).round() as i64)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum DetectionInputKind {
    /// TCP経由 (camera)
    Network,
    /// ローカルのSQLiteから
    #[cfg(feature = "input-sqlite")]
    Sqlite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum CollisionOutputKind {
    /// TCP経由 (projector)
    Network,
    /// CSVファイルに保存
    Csv,
}

/// [`DetectionInput`] is not dyn compatible, so we need this enum.
enum DetectionInputImpl {
    Network(NetworkDetectionInput),
    #[cfg(feature = "input-sqlite")]
    Sqlite(SqliteDetectionInput),
}

impl DetectionInput for DetectionInputImpl {
    async fn start(self, tx: mpsc::Sender<CameraFrame>) -> std::io::Result<()> {
        match self {
            DetectionInputImpl::Network(input) => input.start(tx).await,
            #[cfg(feature = "input-sqlite")]
            DetectionInputImpl::Sqlite(input) => input.start(tx).await,
        }
    }
}

impl DetectionInputImpl {
    async fn new(
        kind: DetectionInputKind,
        camera_locs: Arc<CameraLocationStore>,
        clock: Arc<ClockSync>,
        video: Option<VideoOptions>,
        cameras: Option<Arc<CameraConfig>>,
    ) -> Self {
        match kind {
            DetectionInputKind::Network => DetectionInputImpl::Network(
                NetworkDetectionInput::new(camera_locs, clock, video, cameras)
                    .await
                    .unwrap(),
            ),
            #[cfg(feature = "input-sqlite")]
            DetectionInputKind::Sqlite => DetectionInputImpl::Sqlite(SqliteDetectionInput {}),
        }
    }
}

enum CollisionOutputImpl {
    Network(NetworkCollisionOutput),
    Csv(CsvCollisionOutput),
}

impl CollisionOutput for CollisionOutputImpl {
    async fn start(self, collision_rx: mpsc::Receiver<CollisionPoint3D>) {
        match self {
            CollisionOutputImpl::Network(output) => output.start(collision_rx).await,
            CollisionOutputImpl::Csv(output) => output.start(collision_rx).await,
        }
    }
}

impl CollisionOutputImpl {
    async fn new(cli: &Cli) -> Self {
        match &cli.collision_output {
            CollisionOutputKind::Network => {
                Self::Network(NetworkCollisionOutput::bind().await.unwrap())
            }
            CollisionOutputKind::Csv => {
                // OPTIM: 無駄なclone
                let path = cli.csv_path.clone().unwrap_or_else(|| csv_path_default());
                Self::Csv(CsvCollisionOutput::new(path))
            }
        }
    }
}

fn csv_path_default() -> PathBuf {
    let mut ret = dirs::config_dir().unwrap();
    ret.push("struckout/");
    ret.push("tracker/");
    ret.push("csv/");
    let fname = format!("{}.csv", Local::now().format("%Y%m%d_%H%M"));
    ret.push(fname);
    ret
}

enum EventLoggerImpl {
    Json(JsonEventLogger),
    #[allow(dead_code)] // 後で追加する
    Sentry(SentryEventLogger),
    Fmt(FmtEventLogger),
}

impl EventLoggerImpl {
    fn new(cli: &Cli) -> Self {
        if cli.output_to_json {
            Self::Json(JsonEventLogger::new(json_log_output_dir()))
        //}else if  {
        //    Self::Sentry(SentryEventLogger::new())
        } else {
            Self::Fmt(FmtEventLogger)
        }
    }
}

impl EventLogger for EventLoggerImpl {
    fn push_events(&mut self, events: TrackingEventsDto) {
        match self {
            EventLoggerImpl::Json(l) => l.push_events(events),
            EventLoggerImpl::Sentry(l) => l.push_events(events),
            EventLoggerImpl::Fmt(l) => l.push_events(events),
        }
    }

    fn push_frame(&mut self, frame: &CameraFrame) {
        match self {
            EventLoggerImpl::Json(l) => l.push_frame(frame),
            EventLoggerImpl::Sentry(l) => l.push_frame(frame),
            EventLoggerImpl::Fmt(l) => l.push_frame(frame),
        }
    }
}

fn json_log_output_dir() -> PathBuf {
    let mut ret = dirs::config_dir().unwrap();
    ret.push("struckout/");
    ret.push("tracker/");
    ret.push("json/");
    ret
}

fn main() {
    #[cfg(feature = "sentry")]
    let _guard = sentry::init(
        sentry::ClientOptions::new()
            .dsn(SENTRY_DSN)
            .maybe_release(sentry::release_name!())
            .traces_sample_rate(1.0),
    );

    let reg = tracing_subscriber::registry().with(tracing_subscriber::fmt::layer());
    #[cfg(feature = "sentry")]
    let reg = reg.with(sentry::integrations::tracing::layer());
    reg.init();

    #[cfg(feature = "sentry")]
    tracing::info!("initialized tracing with sentry layer");

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let fut = run_main();
            #[cfg(feature = "sentry")]
            let fut = {
                use sentry::SentryFutureExt;
                fut.bind_hub(sentry::Hub::current())
            };
            fut.await
        });
}

async fn run_main() {
    let cli = parse_cli();

    let camera_locs = Arc::new(CameraLocationStore::new());
    let cameras = cli.cameras.as_ref().map(|path| {
        let config = CameraConfig::load(path).unwrap();
        // 位置は最初から分かっているので、つなぐ前から入れておく (左右もこれで決まる)
        for (camera_id, entry) in config.entries().iter().enumerate() {
            tracing::info!(
                camera_id,
                name = entry.name,
                device_id = entry.device_id.as_deref().unwrap_or(""),
                pose = ?entry.pose,
                focal_ratio = entry.focal_ratio,
                "loaded camera settings"
            );
            camera_locs.insert(CameraId::new(camera_id as u32), entry.camera_pose());
        }
        Arc::new(config)
    });

    let auto_trim = Arc::new(AutoTimeTrim::new(cli.auto_time_trim));
    let clock = Arc::new(
        ClockSync::new(
            camera_locs.clone(),
            ClockSyncOptions {
                time_trim: millis(cli.time_trim_ms),
                rolling_shutter_skew: cli.rolling_shutter_skew_ms.map(millis),
            },
        )
        .with_auto_trim(auto_trim.clone()),
    );

    let video = (!cli.no_video_analysis).then(|| VideoOptions {
        hwaccel: video::hwaccel::select(&cli.ffmpeg, &cli.video_hwaccel),
        detection: video::analysis::DetectionOptions {
            center: cli.detection_center,
            edge: cli.edge_detections,
        },
        ffmpeg: cli.ffmpeg.clone(),
        snapshot_dir: cli.video_snapshot_dir.clone(),
        record_dir: cli.video_record_dir.clone(),
    });
    let detection_input = DetectionInputImpl::new(
        cli.detection_input,
        camera_locs.clone(),
        clock,
        video,
        cameras,
    )
    .await;
    let collision_output = CollisionOutputImpl::new(&cli).await;
    let event_logger = EventLoggerImpl::new(&cli);

    let app = Application::new(
        detection_input,
        collision_output,
        camera_locs.clone(),
        event_logger,
        millis(cli.report_delay_ms),
        auto_trim,
    );

    app.run().await.unwrap();
}

/// コマンドライン引数に、設定ファイルから作った引数を足して解釈する
fn parse_cli() -> Cli {
    let args = std::env::args_os().collect::<Vec<_>>();
    let location = ConfigLocation::from_args(&args);
    let config_args = match location.as_ref().map(config_file::load_args).transpose() {
        Ok(config_args) => config_args.flatten(),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    let loaded = config_args.is_some();
    Cli::try_parse_from(merge_args(args, config_args.unwrap_or_default())).unwrap_or_else(|error| {
        let _ = error.print();
        if let (Some(location), true) = (location, loaded)
            && error.use_stderr()
        {
            eprintln!(
                "(設定ファイル {} の中身も引数として読んでいます)",
                location.path().display()
            );
        }
        std::process::exit(error.exit_code());
    })
}

/// 設定ファイルの引数を、プログラム名のすぐ後ろ (コマンドラインの引数より前) に置く。
/// `args_override_self` で後の値が勝つので、同じ引数はコマンドラインのほうが優先される
fn merge_args(args: Vec<OsString>, config_args: Vec<OsString>) -> Vec<OsString> {
    let mut args = args.into_iter();
    args.next()
        .into_iter()
        .chain(config_args)
        .chain(args)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os_args(args: &[&str]) -> Vec<OsString> {
        args.iter().map(OsString::from).collect()
    }

    #[test]
    fn command_line_overrides_config_file() {
        let cli = Cli::try_parse_from(merge_args(
            os_args(&[
                "ball_tracker",
                "--output",
                "network",
                "--report-delay-ms",
                "10",
            ]),
            os_args(&["--input=network", "--output=csv", "--report-delay-ms=-30"]),
        ))
        .unwrap();
        assert_eq!(cli.detection_input, DetectionInputKind::Network);
        assert_eq!(cli.collision_output, CollisionOutputKind::Network);
        assert_eq!(cli.report_delay_ms, 10.0);
    }

    #[test]
    fn config_file_can_give_required_arguments() {
        let cli = Cli::try_parse_from(merge_args(
            os_args(&["ball_tracker"]),
            os_args(&["--input=network", "--output=csv", "--json"]),
        ))
        .unwrap();
        assert_eq!(cli.collision_output, CollisionOutputKind::Csv);
        assert!(cli.output_to_json);
    }
}
