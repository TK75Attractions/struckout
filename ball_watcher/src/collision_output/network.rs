use struckout_proto::{CollisionPoint, ProjectorPacket, projector_packet, write_packet};
use tokio::{net::TcpStream, sync::mpsc};
use tracing::error;

use crate::{collision_output::CollisionOutput, types::CollisionPoint3D};

// TODO: set actual value
const PROJECTOR_ADDR: &str = "192.168.00.000";

pub struct NetworkCollisionOutput {
    stream: TcpStream,
}

impl NetworkCollisionOutput {
    pub async fn connect() -> Result<Self, std::io::Error> {
        let stream = TcpStream::connect(PROJECTOR_ADDR).await?;
        Ok(Self { stream })
    }
}

impl CollisionOutput for NetworkCollisionOutput {
    async fn start(mut self, mut collision_rx: mpsc::Receiver<CollisionPoint3D>) {
        loop {
            let coll = match collision_rx.recv().await {
                Some(c) => c,
                None => {
                    error!("collision channel has been unexpectedly closed");
                    std::process::exit(1);
                }
            };
            let packet = ProjectorPacket {
                payload: Some(projector_packet::Payload::Point(CollisionPoint {
                    x: coll.x,
                    y: coll.z, // FIXME: これあってる?
                })),
            };
            write_packet(packet, &mut self.stream).await.unwrap();
        }
    }
}
