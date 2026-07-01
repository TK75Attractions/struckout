use tokio::{net::TcpStream, sync::mpsc};
use tracing::error;

use crate::{
    protobuf::{self, ProjectorPacket, projector_packet},
    transport::write_packet,
    types::CollisionPoint3D,
};

// TODO: set actual value
const PROJECTOR_ADDR: &str = "192.168.00.000";

async fn collision_sender(mut collision_rx: mpsc::Receiver<CollisionPoint3D>) {
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
            payload: Some(projector_packet::Payload::Point(protobuf::CollisionPoint {
                x: coll.x,
                y: coll.z, // FIXME: これあってる?
            })),
        };
        write_packet(packet, &mut socket).await.unwrap();
    }
}
