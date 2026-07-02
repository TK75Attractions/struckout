#![allow(dead_code)] // temporary allow dead code
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};

use anyhow::Context;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use tokio::sync::mpsc;

use crate::{
    collision_output::CollisionOutput,
    detection_input::DetectionInput,
    tracking::KalmanTrack,
    types::{CameraId, CollisionPoint3D},
};
use struckout_proto::{CameraLocation, UdpPacket};

pub mod collision_output;
pub mod detection_input;
mod tracking;
pub(crate) mod types;

const FRAME_CHANNEL_BUF: usize = 16;
const COLLISION_CHANNEL_BUF: usize = 16;

pub struct Application<DI, CO> {
    detection_input: DI,
    collision_output: CO,
    state: Arc<RwLock<State>>,
}

impl<DI, CO> Application<DI, CO>
where
    DI: DetectionInput + Send + 'static,
    CO: CollisionOutput + Send + 'static,
{
    pub fn new(detection_input: DI, collision_output: CO, state: Arc<RwLock<State>>) -> Self {
        Self {
            detection_input,
            collision_output,
            state,
        }
    }

    pub async fn run(self) -> anyhow::Result<()> {
        let (frame_tx, frame_rx) = mpsc::channel(FRAME_CHANNEL_BUF);
        let (collision_tx, collision_rx) = mpsc::channel(COLLISION_CHANNEL_BUF);

        let detection_input = self.detection_input;
        let join1 = tokio::spawn(async move {
            detection_input.start(frame_tx).await.unwrap();
        });
        let join2 = tokio::spawn(collect_frames(self.state, frame_rx, collision_tx));
        let join3 = tokio::spawn(async move {
            self.collision_output.start(collision_rx).await;
        });
        join1.await.unwrap();
        join2.await.unwrap();
        join3.await.unwrap();
        anyhow::Ok(())
    }
}

/// Holds application states.
pub struct State {
    frames: VecDeque<(DateTime<Utc>, UdpPacket)>,
    camera_locs: HashMap<CameraId, CameraLocation>,
    tracks: Vec<KalmanTrack>,
}

impl State {
    pub fn new() -> Self {
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
