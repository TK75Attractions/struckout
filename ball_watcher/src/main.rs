use std::sync::Arc;

use ball_watcher::{
    Application, State, collision_output::NetworkCollisionOutput,
    detection_input::NetworkDetectionInput, tracking::KalmanTrack,
};
use parking_lot::RwLock;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();
    tracing::subscriber::set_global_default(subscriber).unwrap();

    let state = Arc::new(RwLock::new(State::new()));
    let detection_input = NetworkDetectionInput::new(Arc::clone(&state))
        .await
        .unwrap();
    let collision_output = NetworkCollisionOutput::connect().await.unwrap();

    let app = Application::<KalmanTrack<Arc<RwLock<State>>>, _, _, _>::new(
        detection_input,
        collision_output,
        Arc::clone(&state),
    );

    app.run().await.unwrap();
}
