use std::{path::PathBuf, sync::Arc};

#[cfg(feature = "input-sqlite")]
use ball_tracker::detection_input::SqliteDetectionInput;
use ball_tracker::{
    Application, CameraLocationStore,
    clock_sync::{ClockSync, ClockSyncOptions},
    collision_output::{CollisionOutput, CsvCollisionOutput, NetworkCollisionOutput},
    detection_input::{CameraFrame, DetectionInput, NetworkDetectionInput},
    tracking::{
        EventLogger, FmtEventLogger, JsonEventLogger, SentryEventLogger, TrackingEventsDto,
    },
    types::CollisionPoint3D,
};
use chrono::{Local, TimeDelta};
use clap::{Parser, ValueEnum};
use tokio::sync::mpsc;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[derive(Parser)]
struct Cli {
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
    #[arg(long = "time-trim-ms", default_value_t = 0.0, allow_negative_numbers = true)]
    time_trim_ms: f64,
    /// 画像の上端の行と下端の行の露光開始の時刻の差 (ms)。指定すると、カメラが知らせた値の代わりに使う
    #[arg(long = "rolling-shutter-skew-ms", allow_negative_numbers = true)]
    rolling_shutter_skew_ms: Option<f64>,
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
    ) -> Self {
        match kind {
            DetectionInputKind::Network => DetectionInputImpl::Network(
                NetworkDetectionInput::new(camera_locs, clock)
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
                Self::Network(NetworkCollisionOutput::connect().await.unwrap())
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

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();
    tracing::subscriber::set_global_default(subscriber).unwrap();

    let camera_locs = Arc::new(CameraLocationStore::new());

    let clock = Arc::new(ClockSync::new(
        camera_locs.clone(),
        ClockSyncOptions {
            time_trim: millis(cli.time_trim_ms),
            rolling_shutter_skew: cli.rolling_shutter_skew_ms.map(millis),
        },
    ));

    let detection_input =
        DetectionInputImpl::new(cli.detection_input, camera_locs.clone(), clock).await;
    let collision_output = CollisionOutputImpl::new(&cli).await;
    let event_logger = EventLoggerImpl::new(&cli);

    let app = Application::new(
        detection_input,
        collision_output,
        camera_locs.clone(),
        event_logger,
    );

    app.run().await.unwrap();
}
