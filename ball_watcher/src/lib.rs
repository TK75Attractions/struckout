#![allow(dead_code)] // temporary allow dead code
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};

use anyhow::Context;
use chrono::{DateTime, TimeDelta, TimeZone, Utc};
use parking_lot::RwLock;
use tokio::{net::TcpStream, sync::mpsc};
use tracing::{error, warn};

use crate::{
    data_association::associate_objects,
    kalman::KalmanTrack,
    protobuf::{CameraLocation, ProjectorPacket, UdpPacket, projector_packet},
    transport::{TcpTransport, UdpTransport, write_packet},
    triangulate::triangulate,
    types::{CameraId, CollisionPoint3D, GetLayFromDetectedObject},
};

mod collision_output;
pub(crate) mod data_association;
mod detection_input;
pub(crate) mod kalman;
mod tracking;
pub(crate) mod transport;
pub(crate) mod triangulate;
pub(crate) mod types;

pub mod protobuf {
    include!(concat!(env!("OUT_DIR"), "/tk75attractions.struckout.v1.rs"));
}

const FRAME_CHANNEL_BUF: usize = 16;
const COLLISION_CHANNEL_BUF: usize = 16;

pub async fn run_main() -> std::io::Result<()> {
    let state = Arc::new(RwLock::new(State::new()));

    let (frame_tx, frame_rx) = mpsc::channel(FRAME_CHANNEL_BUF);
    let (collision_tx, mut collision_rx) = mpsc::channel(COLLISION_CHANNEL_BUF);
    let mut camera_loc_listener = TcpTransport::new(Arc::clone(&state)).await?;
    let mut frame_socket = UdpTransport::new(frame_tx).await?;

    let join1 = tokio::spawn(async move {
        camera_loc_listener.listen().await;
    });
    let join2 = tokio::spawn(async move {
        frame_socket.start().await.unwrap();
    });
    let join3 = tokio::spawn(collect_frames(state, frame_rx, collision_tx));
    #[cfg(feature = "network-projector")]
    let join4 = tokio::spawn(collision_sender(collision_rx));
    #[cfg(not(feature = "network-projector"))]
    let join4 = tokio::spawn(async move {
        loop {
            use tracing::trace;

            let coll = collision_rx.recv().await.unwrap();
            trace!(target = "collision", ?coll, "detected collision");
        }
    });
    join1.await.unwrap();
    join2.await.unwrap();
    join3.await.unwrap();
    join4.await.unwrap();

    Ok(())
}

/// Holds application states.
struct State {
    frames: VecDeque<(DateTime<Utc>, UdpPacket)>,
    camera_locs: HashMap<CameraId, CameraLocation>,
    tracks: Vec<KalmanTrack>,
}

impl State {
    fn new() -> Self {
        Self {
            frames: VecDeque::new(),
            camera_locs: HashMap::new(),
            tracks: Vec::new(),
        }
    }
}

async fn collect_frames(
    state: Arc<RwLock<State>>,
    mut frame_rx: mpsc::Receiver<UdpPacket>,
    collision_tx: mpsc::Sender<CollisionPoint3D>,
) {
    loop {
        let frame = frame_rx.recv().await;
        let frame = frame
            .with_context(|| "frame channel is unexpectedly closed")
            .unwrap();
        let collisions = state.write().update_frame(frame);
        for coll in collisions {
            collision_tx.send(coll).await.unwrap();
        }
    }
}

const FRAME_MATCHING_DELTA: TimeDelta = TimeDelta::milliseconds(3);

impl State {
    fn update_frame(&mut self, packet: UdpPacket) -> Vec<CollisionPoint3D> {
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
                return Vec::new();
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
        let pair = PairedFrames::new(a, b);

        let assignments = associate_objects(&mut self.tracks, &pair);
        // known tracks
        let res = self.update_assigned_tracks(&pair, &assignments);
        for (track_idx, _) in &res.collisions {
            self.tracks.remove(*track_idx);
        }

        // dropped tracks
        assignments
            .iter()
            .filter_map(|(track, (a, b))| {
                if a.is_none() && b.is_none() {
                    Some(track)
                } else {
                    None
                }
            })
            .for_each(|track_idx| todo!("dropped track: {}", track_idx));

        // new track
        // TODO

        res.collisions.into_iter().map(|(_, coll)| coll).collect()
    }

    /// Updates tracks based on assignment.
    fn update_assigned_tracks(
        &mut self,
        pair: &PairedFrames,
        assignments: &HashMap<usize, (Option<usize>, Option<usize>)>,
    ) -> UpdateTrackResult {
        let mut assigned_dets_a = Vec::new();
        let mut assigned_dets_b = Vec::new();
        let mut collisions = Vec::new();
        assignments
            .iter()
            .filter_map(|(track, (a, b))| {
                if a.is_some() && b.is_some() {
                    Some((track, (a.unwrap(), b.unwrap())))
                } else {
                    None
                }
            })
            .for_each(|(track_idx, (det_a, det_b))| {
                assigned_dets_a.push(det_a);
                assigned_dets_b.push(det_b);
                let new_pos = triangulate(
                    self.camera_locs.get(&CameraId::new(0)).unwrap().clone(),
                    pair.a.detected_objects.get(det_a).unwrap().get_lay(),
                    self.camera_locs.get(&CameraId::new(1)).unwrap().clone(),
                    pair.b.detected_objects.get(det_b).unwrap().get_lay(),
                );
                let track = self.tracks.get_mut(*track_idx).unwrap();
                let coll = track.update_and_check_collision(new_pos);
                if let Some(coll) = coll {
                    collisions.push((*track_idx, coll));
                }
            });
        UpdateTrackResult {
            assigned_dets_a,
            assigned_dets_b,
            collisions,
        }
    }
}

/// Result of [`State::update_assigned_tracks()`]
#[derive(Debug)]
struct UpdateTrackResult {
    assigned_dets_a: Vec<usize>,
    assigned_dets_b: Vec<usize>,
    /// (`track_idx`, `collision_point`)
    collisions: Vec<(usize, CollisionPoint3D)>,
}

/// Paired frames from two cameras at the same timestamp.
#[derive(Debug)]
struct PairedFrames {
    timestamp_avr: DateTime<Utc>,
    a: UdpPacket,
    b: UdpPacket,
}

impl PairedFrames {
    fn new(a: (DateTime<Utc>, UdpPacket), b: (DateTime<Utc>, UdpPacket)) -> Self {
        Self {
            timestamp_avr: b.0 + (a.0 - b.0) / 2,
            a: a.1,
            b: b.1,
        }
    }
}
