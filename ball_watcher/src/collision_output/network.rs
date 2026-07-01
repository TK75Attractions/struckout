use struckout_proto::{CollisionPoint, ProjectorPacket, projector_packet, write_packet};
use tokio::{net::TcpStream, sync::mpsc};
use tracing::error;

use crate::types::CollisionPoint3D;

// TODO: set actual value
const PROJECTOR_ADDR: &str = "192.168.00.000";

pub async fn collision_sender(mut collision_rx: mpsc::Receiver<CollisionPoint3D>) {
    let mut socket = TcpStream::connect(PROJECTOR_ADDR)
        .await
        .inspect_err(|e| error!(err = ?e,"failed to connect to projector"))
        .unwrap();
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
        write_packet(packet, &mut socket).await.unwrap();
    }
}
