use struckout_proto::UdpPacket;
use tokio::sync::mpsc;

mod network;
pub use network::{NetworkDetectionInput, NetworkDetectionInputCreationError};
mod sqlite;

pub trait DetectionInput {
    fn start(
        self,
        tx: mpsc::Sender<UdpPacket>,
    ) -> impl std::future::Future<Output = std::io::Result<()>> + Send;
}
