mod network;
pub use network::NetworkCollisionOutput;
use tokio::sync::mpsc;

use crate::types::CollisionPoint3D;
mod json;

pub trait CollisionOutput {
    fn start(
        self,
        collision_rx: mpsc::Receiver<CollisionPoint3D>,
    ) -> impl std::future::Future<Output = ()> + Send + 'static;
}
