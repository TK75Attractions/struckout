use std::sync::Arc;

use ball_watcher::{
    Application, State, collision_output::NetworkCollisionOutput,
    detection_input::NetworkDetectionInput, tracking::KalmanTrack,
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

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();
    tracing::subscriber::set_global_default(subscriber).unwrap();

    let state = Arc::new(RwLock::new(State::new()));

    let detection_input = match cli.detection_input {
        DetectionInputKind::Network => NetworkDetectionInput::new(Arc::clone(&state))
            .await
            .unwrap(),
        DetectionInputKind::Sqlite => todo!(),
    };
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
