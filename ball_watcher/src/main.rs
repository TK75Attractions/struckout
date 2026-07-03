use std::sync::Arc;

use ball_watcher::{
    Application, State,
    collision_output::NetworkCollisionOutput,
    detection_input::{DetectionInput, NetworkDetectionInput, SqliteDetectionInput},
    tracking::KalmanTrack,
};
use clap::{Parser, ValueEnum};
use parking_lot::RwLock;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[derive(Parser)]
struct Cli {
    #[arg(value_enum, long = "input", help = "detectionをどこから受け取るか")]
    detection_input: DetectionInputKind,
    #[arg(value_enum, long = "output", help = "collisionをどこに送信するか")]
    collision_output: CollisionOutputKind,
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
        tx: tokio::sync::mpsc::Sender<ball_watcher::detection_input::PairedFrames>,
    ) -> std::io::Result<()> {
        match self {
            DetectionInputImpl::Network(input) => input.start(tx).await,
            DetectionInputImpl::Sqlite(input) => input.start(tx).await,
        }
    }
}

impl DetectionInputImpl {
    async fn new(kind: DetectionInputKind, state: Arc<RwLock<State>>) -> Self {
        match kind {
            DetectionInputKind::Network => {
                DetectionInputImpl::Network(NetworkDetectionInput::new(state).await.unwrap())
            }
            DetectionInputKind::Sqlite => DetectionInputImpl::Sqlite(SqliteDetectionInput {}),
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

    let state = Arc::new(RwLock::new(State::new()));

    let detection_input = DetectionInputImpl::new(cli.detection_input, Arc::clone(&state)).await;
    let collision_output = match cli.collision_output {
        CollisionOutputKind::Network => NetworkCollisionOutput::connect().await.unwrap(),
        CollisionOutputKind::Csv => todo!(),
    };

    let app = Application::<KalmanTrack<Arc<RwLock<State>>>, _, _, _>::new(
        detection_input,
        collision_output,
        Arc::clone(&state),
    );

    app.run().await.unwrap();
}
