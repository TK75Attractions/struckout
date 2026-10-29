#![allow(dead_code)] // temporary allow dead code
use std::{collections::HashMap, marker::Send, sync::Arc};

use parking_lot::RwLock;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::{
    collision_output::CollisionOutput, detection_input::DetectionInput, tracking::TrackRunner,
    types::CameraId,
};
use struckout_proto::CameraPose;

pub mod camera_config;
pub mod camera_status;
pub mod clock_sync;
pub mod collision_output;
pub mod detection_input;
pub mod tracking;
pub mod types;
pub mod video;

const FRAME_CHANNEL_BUF: usize = 16;
const COLLISION_CHANNEL_BUF: usize = 16;

pub struct Application<DI, CO, EL> {
    detection_input: DI,
    collision_output: CO,
    track_runner: TrackRunner<EL>,
    camera_locs: Arc<CameraLocationStore>,
}

impl<DI, CO, EL> Application<DI, CO, EL>
where
    DI: DetectionInput + Send + 'static,
    CO: CollisionOutput + Send + 'static,
    EL: tracking::EventLogger + Send + 'static,
{
    pub fn new(
        detection_input: DI,
        collision_output: CO,
        camera_locs: Arc<CameraLocationStore>,
        event_logger: EL,
        report_delay: chrono::TimeDelta,
        auto_trim: Arc<clock_sync::AutoTimeTrim>,
    ) -> Self {
        let track_runner = TrackRunner::new(camera_locs.clone(), event_logger)
            .with_report_delay(report_delay)
            .with_auto_trim(auto_trim);
        Self {
            detection_input,
            collision_output,
            track_runner,
            camera_locs,
        }
    }

    pub async fn run(self, shutdown: CancellationToken) -> anyhow::Result<()> {
        let (frame_tx, frame_rx) = mpsc::channel(FRAME_CHANNEL_BUF);
        let (collision_tx, collision_rx) = mpsc::channel(COLLISION_CHANNEL_BUF);

        let join1 = tokio::spawn(async move {
            self.detection_input
                .start(frame_tx, shutdown)
                .await
                .unwrap();
        });
        let join2 = tokio::spawn(async move {
            self.track_runner
                .start(frame_rx, collision_tx)
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

#[derive(Debug)]
pub struct CameraLocationStore(RwLock<HashMap<CameraId, CameraPose>>);

impl CameraLocationStore {
    pub fn new() -> Self {
        Self(RwLock::new(HashMap::new()))
    }

    fn get(&self, id: CameraId) -> Option<CameraPose> {
        self.0.read().get(&id).cloned()
    }

    pub fn insert(&self, id: CameraId, pose: CameraPose) {
        self.0.write().insert(id, pose);
    }

    /// 登録された位置の y が最も大きいカメラ。プレイヤーから見て右のカメラにあたる。
    /// 2 台そろうまでは決まらない。
    fn rightmost(&self) -> Option<CameraId> {
        let poses = self.0.read();
        if poses.len() < 2 {
            return None;
        }
        poses
            .iter()
            .max_by(|a, b| a.1.y.total_cmp(&b.1.y))
            .map(|(id, _)| *id)
    }
}

impl Default for CameraLocationStore {
    fn default() -> Self {
        Self::new()
    }
}
