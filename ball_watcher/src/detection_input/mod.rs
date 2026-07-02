use struckout_proto::UdpPacket;
use tokio::sync::mpsc;

mod network;
mod sqlite;

pub trait DetectionInput {
    fn start(
        &mut self,
        tx: mpsc::Sender<UdpPacket>,
    ) -> impl std::future::Future<Output = std::io::Result<()>> + Send;
}
