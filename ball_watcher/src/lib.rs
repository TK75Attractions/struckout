#![allow(dead_code)] // temporary allow dead code
use std::{collections::HashMap, marker::Send, sync::Arc};

use parking_lot::RwLock;
use tokio::sync::mpsc;

use crate::{
    collision_output::CollisionOutput,
    detection_input::DetectionInput,
    tracking::{ObjectTrack, TrackRunner},
    types::CameraId,
};
use struckout_proto::CameraLocation;

pub mod collision_output;
pub mod detection_input;
pub mod tracking;
pub mod types;

const FRAME_CHANNEL_BUF: usize = 16;
const COLLISION_CHANNEL_BUF: usize = 16;

pub struct Application<T, P, DI, CO> {
    detection_input: DI,
    collision_output: CO,
    track_runner: TrackRunner<T, P>,
    state: P,
}

impl<T, P, DI, CO> Application<T, P, DI, CO>
where
    T: ObjectTrack + Send + 'static,
    P: CameraLocationProvider,
    DI: DetectionInput + Send + 'static,
    CO: CollisionOutput + Send + 'static,
{
    pub fn new(detection_input: DI, collision_output: CO, camera_locs: P) -> Self {
        let track_runner = TrackRunner::new(camera_locs.clone());
        Self {
            detection_input,
            collision_output,
            track_runner,
            state: camera_locs,
        }
    }

    pub async fn run(self) -> anyhow::Result<()> {
        let (pair_tx, pair_rx) = mpsc::channel(FRAME_CHANNEL_BUF);
        let (collision_tx, collision_rx) = mpsc::channel(COLLISION_CHANNEL_BUF);

        let join1 = tokio::spawn(async move {
            self.detection_input.start(pair_tx).await.unwrap();
        });
        let join2 = tokio::spawn(async move {
            self.track_runner
                .start(pair_rx, collision_tx)
                .await
                .unwrap();
        });
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

pub struct CameraLocationStore(RwLock<HashMap<CameraId, CameraLocation>>);

impl CameraLocationStore {
    pub fn new() -> Self {
        Self(RwLock::new(HashMap::new()))
    }
}

pub trait CameraLocationProvider: Send + 'static + Clone {
    fn get(&self, id: CameraId) -> Option<CameraLocation>;
    fn insert(&self, id: CameraId, loc: CameraLocation);
    fn next(&self) -> usize;
}

impl CameraLocationProvider for Arc<CameraLocationStore> {
    fn get(&self, id: CameraId) -> Option<CameraLocation> {
        self.0.read().get(&id).cloned()
    }

    fn insert(&self, id: CameraId, loc: CameraLocation) {
        self.0.write().insert(id, loc);
    }

    fn next(&self) -> usize {
        self.0.read().len()
    }
}
