mod network;
use std::future::Future;

pub use network::NetworkCollisionOutput;
use tokio::sync::mpsc;

use crate::types::CollisionPoint3D;
mod csv;
pub use csv::CsvCollisionOutput;

pub trait CollisionOutput {
    fn start(
        self,
        collision_rx: mpsc::Receiver<CollisionPoint3D>,
    ) -> impl Future<Output = ()> + Send + 'static;
}
