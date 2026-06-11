use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
    time::Duration,
};

use anyhow::Context;
use tokio::{
    select,
    sync::{RwLock, mpsc},
};

use crate::{
    protobuf::{CameraLocation, UdpPacket},
    transport::{TcpTransport, UdpTransport},
    types::CameraId,
};

pub(crate) mod transport;
pub(crate) mod triangulate;
pub(crate) mod types;

pub mod protobuf {
    include!(concat!(env!("OUT_DIR"), "/struckout.v1.rs"));
}

pub async fn run_main() -> std::io::Result<()> {
    let state = Arc::new(RwLock::new(State::new()));

    let (frame_tx, frame_rx) = mpsc::channel(16);
    let mut camera_loc_listener = TcpTransport::new(Arc::clone(&state)).await?;
    let mut frame_socket = FrameSocket::new(frame_tx).await?;

    let join1 = tokio::spawn(async move {
        camera_loc_listener.listen().await;
    });
    let join2 = tokio::spawn(async move {
        frame_socket.start().await.unwrap();
    });
    join1.await.unwrap();
    join2.await.unwrap();
    Ok(())
}

struct State {
    frames: BTreeMap<Duration, CollectedFrame>,
    camera_locs: HashMap<CameraId, CameraLocation>,
}

struct CollectedFrame {
    frame_a: Option<UdpPacket>,
    frame_b: Option<UdpPacket>,
}

impl State {
    fn new() -> Self {
        Self {
            frames: BTreeMap::new(),
            camera_locs: HashMap::new(),
        }
    }
}

async fn collect_frames(mut state: State, mut frame_rx: mpsc::Receiver<UdpPacket>) {
    loop {
        select! {
            frame = frame_rx.recv() => {
                let frame = frame
                    .with_context(|| "frame channel is unexpectedly closed")
                    .unwrap();
                update_frame(&mut state,frame)
            }
        }
    }
}

fn update_frame(state: &mut State, packet: UdpPacket) {
    todo!()
}
