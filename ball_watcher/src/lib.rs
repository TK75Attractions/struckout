use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    hash::Hash,
    sync::Arc,
    time::Duration,
};

use anyhow::Context;
use chrono::{DateTime, TimeDelta, TimeZone, Utc};
use tokio::{
    select,
    sync::{RwLock, mpsc},
};
use tracing::warn;

use crate::{
    kalman::{ObjectId, ObjectTrackerKalman},
    protobuf::{CameraLocation, UdpPacket},
    transport::{TcpTransport, UdpTransport},
    types::CameraId,
};

pub(crate) mod kalman;
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
        update_frame(&mut *state.write().await, frame)
    }
}

const FRAME_MATCHING_DELTA: TimeDelta = TimeDelta::milliseconds(3);

impl State {
    async fn update_frame(&mut self, packet: UdpPacket) {
        let cur_frame_time = Utc.timestamp_opt(packet.timestamp, 0).unwrap();
        let cur_frame_cam_id = packet.camera_id;
        self.frames.push_back((cur_frame_time, packet));

        // match corresponding frames
        let idx = {
            let mut recent_frames = self.frames.iter().enumerate().filter(|(_, (t, p))| {
                *t - cur_frame_time < FRAME_MATCHING_DELTA && p.camera_id != cur_frame_cam_id
            });
            let Some((idx, recent_frame)) = recent_frames.next() else {
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
        let b = self.frames.remove(idx).unwrap();
        let pair = PairedFrames {
            timestamp_avr: b.0 + (a.0 - b.0) / 2,
            a: a.1,
            b: b.1,
        };

        // (obj_id -> score) per object snapshot
        let mut scores_all: Vec<HashMap<ObjectId, f32>> =
            vec![HashMap::new(); pair.a.detected_objects.len()];
        for tracker in &mut self.objects {
            let scores = tracker.evaluate_scores(&pair).await;
            scores.enumerate().for_each(|(idx, s)| {
                scores_all.get_mut(idx).unwrap().insert(tracker.obj_id(), s);
            });
        }

        // assign object id based on prior estimate

        // triangulate point

        // update Kalman filter
    }

    fn assign_object_id_to_snapshot(scores_per_snapshots: Vec<HashMap<ObjectId, f32>>) {
        scores_per_snapshots.iter().for_each(|scores| {
            // データの中でのあるオブジェクト(object snapshot)に対して各object trackerが与えたスコアを比較する
            // scores.iter().min_by(|a, b| a.1.total_cmp(b.1));
        });
    }
}

struct PairedFrames {
    timestamp_avr: DateTime<Utc>,
    a: UdpPacket,
    b: UdpPacket,
}
