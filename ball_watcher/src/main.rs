use std::{path::PathBuf, sync::Arc};

use ball_watcher::{
    Application, CameraLocationStore,
    collision_output::{CollisionOutput, CsvCollisionOutput, NetworkCollisionOutput},
    detection_input::{DetectionInput, NetworkDetectionInput, SqliteDetectionInput},
    tracking::{
        EmptyEventLogger, EventLogger, JsonEventLogger, KalmanTrack, SentryEventLogger,
        TrackingEventsDto,
    },
    types::CollisionPoint3D,
};
use chrono::Local;
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum DetectionInputKind {
    /// TCP経由 (camera)
    Network,
    /// ローカルのSQLiteから
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
    Sqlite(SqliteDetectionInput),
}

impl DetectionInput for DetectionInputImpl {
    async fn start(
        self,
        tx: mpsc::Sender<ball_watcher::detection_input::PairedFrames>,
    ) -> std::io::Result<()> {
        match self {
            DetectionInputImpl::Network(input) => input.start(tx).await,
            DetectionInputImpl::Sqlite(input) => input.start(tx).await,
        }
    }
}

impl DetectionInputImpl {
    async fn new(kind: DetectionInputKind, camera_locs: Arc<CameraLocationStore>) -> Self {
        match kind {
            DetectionInputKind::Network => {
                DetectionInputImpl::Network(NetworkDetectionInput::new(camera_locs).await.unwrap())
            }
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
    Empty(EmptyEventLogger),
}

impl EventLoggerImpl {
    fn new(cli: &Cli) -> Self {
        if cli.output_to_json {
            Self::Json(JsonEventLogger::new(json_log_output_dir()))
        //}else if  {
        //    Self::Sentry(SentryEventLogger::new())
        } else {
            Self::Empty(EmptyEventLogger)
        }
    }
}

impl EventLogger for EventLoggerImpl {
    fn push_events(&mut self, events: TrackingEventsDto) {
        match self {
            EventLoggerImpl::Json(l) => l.push_events(events),
            EventLoggerImpl::Sentry(l) => l.push_events(events),
            EventLoggerImpl::Empty(l) => l.push_events(events),
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

    let detection_input = DetectionInputImpl::new(cli.detection_input, camera_locs.clone()).await;
    let collision_output = CollisionOutputImpl::new(&cli).await;
    let event_logger = EventLoggerImpl::new(&cli);

    let app = Application::<KalmanTrack, _, _, _>::new(
        detection_input,
        collision_output,
        camera_locs.clone(),
        event_logger,
    );

    app.run().await.unwrap();
}
