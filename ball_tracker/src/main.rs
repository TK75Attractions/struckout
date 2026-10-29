use std::{path::PathBuf, sync::Arc};

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
use serde::Deserialize;
use tokio::sync::mpsc;
use tracing_subscriber::{
    Layer as _,
    filter::{LevelFilter, Targets},
    layer::SubscriberExt as _,
    util::SubscriberInitExt as _,
};

mod health;

use crate::config::{Config, ConfigLocation};

mod config;

#[cfg(feature = "sentry")]
const SENTRY_DSN: &str = "https://2b9af00fb25f2faa692253f8ed95c43e@o4512229597315072.ingest.us.sentry.io/4512229769871360";

#[derive(Parser)]
#[command(version = version())]
/// どの引数も、設定ファイル (--config) に書いておける。両方にあれば、コマンドラインのほうを使う
// 既定値は、設定ファイルと共通にするため Config の Default にまとめている
struct Cli {
    /// 設定ファイル (YAML) の場所。既定は <設定のフォルダー>/struckout/ball-tracker/config.yaml。
    /// キーは長いオプション名と同じ (README を参照)
    #[arg(long = "config")]
    config: Option<PathBuf>,
    /// detectionをどこから受け取るか。コマンドラインか設定ファイルのどちらかで必ず指定する
    #[arg(value_enum, long = "input")]
    detection_input: Option<DetectionInputKind>,
    /// collisionをどこに送信するか。コマンドラインか設定ファイルのどちらかで必ず指定する
    #[arg(value_enum, long = "output")]
    collision_output: Option<CollisionOutputKind>,
    /// JSONに追跡結果を出力する
    #[arg(short = 'j', long = "json")]
    output_to_json: bool,
    #[arg(short = 'c', long = "csv_path")]
    csv_path: Option<PathBuf>,
    /// 右のカメラ (位置の y が大きいほう) の時刻に足す補正 (ms)。estimate_time_offset.py で求める [既定: 0]
    #[arg(long = "time-trim-ms", allow_negative_numbers = true)]
    time_trim_ms: Option<f64>,
    /// 画像の上端の行と下端の行の露光開始の時刻の差 (ms)。指定すると、カメラが知らせた値の代わりに使う
    #[arg(long = "rolling-shutter-skew-ms", allow_negative_numbers = true)]
    rolling_shutter_skew_ms: Option<f64>,
    /// カメラから動画を受け取って解析するときに使う ffmpeg (PATH にあれば名前だけでよい) [既定: ffmpeg]
    #[arg(long = "ffmpeg")]
    ffmpeg: Option<String>,
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
    /// 的への描画がそのぶん遅れる [既定: -30]
    #[arg(long = "report-delay-ms", allow_negative_numbers = true)]
    report_delay_ms: Option<f64>,
    /// 右のカメラの時刻の補正を、報告した投球から自動で決めて、--time-trim-ms に足す。付けないときは、
    /// 決めた値をログに出すだけ。カメラの位置と向きが合っているときだけ使うこと
    #[arg(long = "auto-time-trim")]
    auto_time_trim: bool,
    /// カメラから受け取った動画のデコードに使う GPU の方式 (ffmpeg の -hwaccel)。auto はその PC で使えるものを
    /// 選ぶ (Windows は d3d11va、cuda、qsv、dxva2 の順)。none は CPU。d3d11va などの名前も指定できる [既定: auto]
    #[arg(long = "video-hwaccel")]
    video_hwaccel: Option<String>,
    /// PC で解析するとき、視線を作る検出の位置。box は枠の中心 (カメラアプリと同じ)、centroid は前景の画素の重心
    /// [既定: box]
    #[arg(long = "detection-center")]
    detection_center: Option<video::analysis::DetectionCenter>,
    /// PC で解析するとき、画像の端に接した検出 (ボールが欠けている) の扱い。keep はそのまま (カメラアプリと同じ)、
    /// drop は捨てる、extrapolate は欠けていない向きの幅から中心を推定する [既定: keep]
    #[arg(long = "edge-detections")]
    edge_detections: Option<video::analysis::EdgeDetections>,
    /// カメラ用スマホごとの設定 (scripts/cameras.json)。書いた端末は、つないだ順によらず書いた順の camera_id
    /// (0, 1, ...) になり、ここに書いた位置と向きを使う。位置と向き、焦点距離、検出の範囲はスマホにも送る
    #[arg(long = "cameras")]
    cameras: Option<PathBuf>,
}

fn version() -> String {
    version_option().unwrap_or("no version".to_string())
}

fn version_option() -> Option<String> {
    if let Some(version) = option_env!("ST_VERSION") {
        let Some(short_hash) = option_env!("ST_COMMIT_SHORT_HASH") else {
            unreachable!("ST_COMMIT_SHORT_HASH should always exist when ST_VERSION is provided")
        };
        Some(format!("{} ({})", version, short_hash))
    } else {
        None
    }
}

fn millis(value: f64) -> TimeDelta {
    TimeDelta::nanoseconds((value * 1e6).round() as i64)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum DetectionInputKind {
    /// TCP経由 (camera)
    Network,
    /// ローカルのSQLiteから
    #[cfg(feature = "input-sqlite")]
    Sqlite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Deserialize)]
#[serde(rename_all = "kebab-case")]
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
    async fn start(
        self,
        tx: mpsc::Sender<CameraFrame>,
        shutdown: tokio_util::sync::CancellationToken,
    ) -> std::io::Result<()> {
        match self {
            DetectionInputImpl::Network(input) => input.start(tx, shutdown).await,
            #[cfg(feature = "input-sqlite")]
            DetectionInputImpl::Sqlite(input) => input.start(tx, shutdown).await,
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
    async fn new(config: &Config) -> Self {
        match config
            .collision_output
            .expect("Config::apply_cli() makes sure that it is set")
        {
            CollisionOutputKind::Network => {
                Self::Network(NetworkCollisionOutput::bind().await.unwrap())
            }
            CollisionOutputKind::Csv => {
                // OPTIM: 無駄なclone
                let path = config.csv_path.clone().unwrap_or_else(csv_path_default);
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
    fn new(config: &Config) -> Self {
        if config.output_to_json {
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
            .maybe_release(version_option())
            .traces_sample_rate(1.0),
    );

    // ランチャーは 1 秒ごとにヘルスを問い合わせる。そのたびに HTTP/2 の TRACE のログが大量に出ないよう、通信の層は INFO までにする
    let transport_logs = Targets::new()
        .with_default(LevelFilter::TRACE)
        .with_targets([
            ("h2", LevelFilter::INFO),
            ("hyper", LevelFilter::INFO),
            ("tower", LevelFilter::INFO),
            ("tonic", LevelFilter::INFO),
        ]);
    let reg = tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_filter(transport_logs));
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
            #[cfg(unix)]
            let mut terminate = tokio::signal::unix::signal(
                tokio::signal::unix::SignalKind::terminate(),
            )
            .expect("SIGTERM handler should be installed");
            let shutdown_signal = async move {
                #[cfg(unix)]
                tokio::select! {
                    result = tokio::signal::ctrl_c() => result.expect("Ctrl-C handler should be installed"),
                    _ = terminate.recv() => {}
                }
                #[cfg(not(unix))]
                tokio::signal::ctrl_c()
                    .await
                    .expect("Ctrl-C handler should be installed");
            };
            let fut = run_main(shutdown_signal);
            #[cfg(feature = "sentry")]
            let fut = {
                use sentry::SentryFutureExt;
                fut.bind_hub(sentry::Hub::current())
            };
            fut.await
        });
}

async fn run_main(shutdown_signal: impl std::future::Future<Output = ()> + Send) {
    let shutdown = tokio_util::sync::CancellationToken::new();
    let config = load_config();

    let health_server = health::start(shutdown.clone()).await;
    let health_reporter = health_server.as_ref().map(|(reporter, _)| reporter);

    let camera_locs = Arc::new(CameraLocationStore::new());
    let cameras = config.cameras.as_ref().map(|path| {
        let camera_config = CameraConfig::load(path).unwrap();
        // 位置は最初から分かっているので、つなぐ前から入れておく (左右もこれで決まる)
        for (camera_id, entry) in camera_config.entries().iter().enumerate() {
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
        Arc::new(camera_config)
    });

    let auto_trim = Arc::new(AutoTimeTrim::new(config.auto_time_trim));
    let clock = Arc::new(
        ClockSync::new(
            camera_locs.clone(),
            ClockSyncOptions {
                time_trim: millis(config.time_trim_ms),
                rolling_shutter_skew: config.rolling_shutter_skew_ms.map(millis),
            },
        )
        .with_auto_trim(auto_trim.clone()),
    );

    let video = (!config.no_video_analysis).then(|| VideoOptions {
        hwaccel: video::hwaccel::select(&config.ffmpeg, &config.video_hwaccel),
        detection: video::analysis::DetectionOptions {
            center: config.detection_center,
            edge: config.edge_detections,
        },
        ffmpeg: config.ffmpeg.clone(),
        snapshot_dir: config.video_snapshot_dir.clone(),
        record_dir: config.video_record_dir.clone(),
    });
    let detection_input = DetectionInputImpl::new(
        config
            .detection_input
            .expect("Config::apply_cli() makes sure that it is set"),
        camera_locs.clone(),
        clock,
        video,
        cameras,
    )
    .await;
    let collision_output = CollisionOutputImpl::new(&config).await;
    let event_logger = EventLoggerImpl::new(&config);

    let app = Application::new(
        detection_input,
        collision_output,
        camera_locs.clone(),
        event_logger,
        millis(config.report_delay_ms),
        auto_trim,
    );

    if let Some(reporter) = health_reporter {
        reporter
            .set_service_status("", tonic_health::ServingStatus::Serving)
            .await;
    }
    let run = app.run(shutdown.clone());
    tokio::pin!(run);
    tokio::select! {
        result = &mut run => result.unwrap(),
        _ = shutdown_signal => {
            tracing::info!("received shutdown signal; stopping the application");
            shutdown.cancel();
            run.await.unwrap();
        }
    }
    shutdown.cancel();
    if let Some((_, server)) = health_server {
        server.await.expect("health server exits after shutdown");
    }
}

/// 設定ファイルを読み、コマンドライン引数で上書きする。誤りがあれば、理由を出して終える
fn load_config() -> Config {
    let cli = Cli::parse();
    let config = ConfigLocation::new(cli.config.clone())
        .map_or_else(
            || Ok(Config::default()),
            |location| Config::from_file(&location),
        )
        .unwrap_or_else(|error| exit_with(&error));
    config
        .apply_cli(cli)
        .unwrap_or_else(|error| exit_with(&error))
}

/// clap が引数の誤りで終えるときと同じく、終了コード 2 で終える
fn exit_with(error: &dyn std::error::Error) -> ! {
    eprintln!("{error}");
    std::process::exit(2);
}
