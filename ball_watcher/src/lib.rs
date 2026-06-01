use std::{
    collections::{BTreeMap, HashMap},
    time::Duration,
};

use anyhow::Context;
use chrono::DateTime;
use tokio::{select, sync::mpsc, time};
use tracing::warn;

use crate::{
    protobuf::{TcpPacket, UdpPacket},
    transport::{CameraLocationListener, FrameSocket},
    types::CameraId,
};

pub(crate) mod transport;
pub(crate) mod triangulate;
pub(crate) mod types;

pub mod protobuf {
    include!(concat!(env!("OUT_DIR"), "/struckout.rs"));
}

pub async fn run_main() -> std::io::Result<()> {
    let (camera_loc_tx, camera_loc_rx) = mpsc::channel(16);
    let (frame_tx, frame_rx) = mpsc::channel(16);
    let mut camera_loc_listener = CameraLocationListener::new(camera_loc_tx).await?;
    let mut frame_socket = FrameSocket::new(frame_tx, time::Duration::from_secs(10)).await?;

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
    camera_locs: HashMap<CameraId, TcpPacket>,
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

async fn collect_frames(
    mut state: State,
    mut frame_rx: mpsc::Receiver<UdpPacket>,
    mut camera_loc_rx: mpsc::Receiver<TcpPacket>,
) {
    loop {
        select! {
            frame = frame_rx.recv() => {
                let frame = frame
                    .with_context(|| "frame channel is unexpectedly closed")
                    .unwrap();
                update_frame(&mut state,frame)
            }
            camera_loc = camera_loc_rx.recv() => {
                let camera_loc = camera_loc
                    .with_context(|| "camera loc channel is unexpectedly closed")
                    .unwrap();
                update_camera_loc(&mut state, camera_loc)
            }
        }
    }
}

fn update_frame(state: &mut State, frame: UdpPacket) {
    if frame.timestamp.is_none() {
        warn!("timestamp was none. camera-side implementation might be incorrect!");
        return;
    }
    let timestamp = frame.timestamp.unwrap(); // checked above
    // This cast is safe because range of Timestamp.nanos is between 0 and 999,999,999.
    // (see https://github.com/protocolbuffers/protobuf/blob/3ddb41be1f87312cd6a9b955dbbfd9730d341dd7/src/google/protobuf/timestamp.proto#L142)
    let date_time = DateTime::from_timestamp(timestamp.seconds, timestamp.nanos as u32).unwrap();
}

fn update_camera_loc(state: &mut State, camera_loc: TcpPacket) {
    let camera_id = CameraId::new(state.camera_locs.len().try_into().unwrap());
    state.camera_locs.insert(camera_id, camera_loc);
}
