use struckout_proto::{CollisionPoint, ProjectorPacket, projector_packet, write_packet};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::mpsc,
};
use tracing::{error, info};

use crate::{collision_output::CollisionOutput, types::CollisionPoint3D};

const PROJECTOR_ADDR: &str = "0.0.0.0:5000";

pub struct NetworkCollisionOutput {
    stream: TcpStream,
}

impl NetworkCollisionOutput {
    pub async fn connect() -> Result<Self, std::io::Error> {
        let listener = TcpListener::bind(PROJECTOR_ADDR).await?;
        info!(addr = PROJECTOR_ADDR, "listening for projector");
        let (stream, addr) = listener.accept().await?;
        info!(?addr, "accepted TCP connection from projector");
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
