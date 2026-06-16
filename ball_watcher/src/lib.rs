#![allow(dead_code)] // temporary allow dead code
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};

use anyhow::Context;
use chrono::{DateTime, TimeDelta, TimeZone, Utc};
use parking_lot::RwLock;
use tokio::sync::mpsc;
use tracing::warn;

use crate::{
    kalman::ObjectTrackerKalman,
    protobuf::{CameraLocation, UdpPacket},
    transport::{TcpTransport, UdpTransport},
    types::CameraId,
};

pub(crate) mod data_association;
pub(crate) mod kalman;
pub(crate) mod transport;
pub(crate) mod triangulate;
pub(crate) mod types;

pub mod protobuf {
    include!(concat!(env!("OUT_DIR"), "/tk75attractions.struckout.v1.rs"));
}

pub async fn run_main() -> std::io::Result<()> {
    let state = Arc::new(RwLock::new(State::new()));

    let (frame_tx, frame_rx) = mpsc::channel(16);
    let mut camera_loc_listener = TcpTransport::new(Arc::clone(&state)).await?;
    let mut frame_socket = UdpTransport::new(frame_tx).await?;

    let join1 = tokio::spawn(async move {
        camera_loc_listener.listen().await;
    });
    let join2 = tokio::spawn(async move {
        frame_socket.start().await.unwrap();
    });
    let join3 = tokio::spawn(collect_frames(state, frame_rx));
    join1.await.unwrap();
    join2.await.unwrap();
    join3.await.unwrap();
    Ok(())
}

struct State {
    frames: VecDeque<(DateTime<Utc>, UdpPacket)>,
    camera_locs: HashMap<CameraId, CameraLocation>,
    objects: Vec<ObjectTrackerKalman>,
}

struct CollectedFrame {
    frame_a: Option<UdpPacket>,
    frame_b: Option<UdpPacket>,
}

impl State {
    fn new() -> Self {
        Self {
            frames: VecDeque::new(),
            camera_locs: HashMap::new(),
            objects: Vec::new(),
        }
    }
}

async fn collect_frames(state: Arc<RwLock<State>>, mut frame_rx: mpsc::Receiver<UdpPacket>) {
    loop {
        let frame = frame_rx.recv().await;
        let frame = frame
            .with_context(|| "frame channel is unexpectedly closed")
            .unwrap();
        state.write().update_frame(frame);
    }
}

const FRAME_MATCHING_DELTA: TimeDelta = TimeDelta::milliseconds(3);

impl State {
    fn update_frame(&mut self, packet: UdpPacket) {
        let cur_frame_time = Utc.timestamp_opt(packet.timestamp, 0).unwrap();
        let cur_frame_cam_id = packet.camera_id;
        self.frames.push_back((cur_frame_time, packet));

        // match corresponding frames
        let idx = {
            let mut recent_frames = self.frames.iter().enumerate().filter(|(_, (t, p))| {
                *t - cur_frame_time < FRAME_MATCHING_DELTA && p.camera_id != cur_frame_cam_id
            });
            let Some((idx, _)) = recent_frames.next() else {
                // wait for another camera to send this frame
                return;
            };
            if recent_frames.next().is_some() {
                warn!(
                    "there was too many frame at the same time. picking first one and ignore others."
                )
            }
            idx
        };

        let a = self.frames.pop_back().unwrap(); // pushed above
        let b = self.frames.remove(idx).unwrap(); // idx comes from above block
        let pair = PairedFrames {
            timestamp_avr: b.0 + (a.0 - b.0) / 2,
            a: a.1,
            b: b.1,
        };

        // assign object id based on prior estimate

        // triangulate point

        // update Kalman filter
    }
}

struct PairedFrames {
    timestamp_avr: DateTime<Utc>,
    a: UdpPacket,
    b: UdpPacket,
}
