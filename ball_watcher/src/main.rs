use std::sync::Arc;

use ball_watcher::{
    Application, CameraLocationStore,
    collision_output::{CollisionOutput, CsvCollisionOutput, NetworkCollisionOutput},
    detection_input::{DetectionInput, NetworkDetectionInput, SqliteDetectionInput},
    tracking::KalmanTrack,
    types::CollisionPoint3D,
};
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
    #[arg(short = 'j', long = "json")]
    output_to_json: bool,
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
    async fn new(kind: CollisionOutputKind) -> Self {
        match kind {
            CollisionOutputKind::Network => {
                Self::Network(NetworkCollisionOutput::connect().await.unwrap())
            }
            CollisionOutputKind::Csv => Self::Csv(CsvCollisionOutput::new(
                "/home/taichi765/source/dev/struckout/ball_watcher/data/hoge.csv",
            )),
        }
    }
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
    let collision_output = CollisionOutputImpl::new(cli.collision_output).await;

    let app = Application::<KalmanTrack<Arc<CameraLocationStore>>, _, _, _>::new(
        detection_input,
        collision_output,
        camera_locs.clone(),
        cli.output_to_json,
    );

    app.run().await.unwrap();
}
