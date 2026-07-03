use std::collections::VecDeque;

use chrono::{DateTime, TimeDelta, TimeZone as _, Utc};
use struckout_proto::UdpPacket;
use tokio::sync::mpsc;

mod db;
mod network;
pub use network::{NetworkDetectionInput, NetworkDetectionInputCreationError};
use tracing::warn;

mod sqlite;
pub use sqlite::SqliteDetectionInput;

pub trait DetectionInput {
    fn start(
        self,
        tx: mpsc::Sender<PairedFrames>,
    ) -> impl std::future::Future<Output = std::io::Result<()>> + Send;
}

const FRAME_MATCHING_DELTA: TimeDelta = TimeDelta::milliseconds(3);

pub struct FramePairMatcher {
    frames: VecDeque<(DateTime<Utc>, UdpPacket)>,
}

impl FramePairMatcher {
    fn new() -> Self {
        Self {
            frames: VecDeque::new(),
        }
    }

    // TODO: テスト書く
    fn pair_frame(&mut self, _time: DateTime<Utc>, packet: UdpPacket) -> Option<PairedFrames> {
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
                return None;
            };
            if recent_frames.next().is_some() {
                warn!(
                    "there was too many frame at the same time. picking first one and ignore others."
                )
            }
            idx
        };

        let (_, a) = self.frames.pop_back().unwrap(); // pushed above
        let (_, b) = self.frames.remove(idx).unwrap(); // idx comes from above block
        Some(PairedFrames::new(a, b))
    }
}

/// Paired frames from two cameras at the same timestamp.
#[derive(Debug)]
pub struct PairedFrames {
    pub timestamp_avr: DateTime<Utc>,
    pub a: UdpPacket,
    pub b: UdpPacket,
}

impl PairedFrames {
    fn new(a: UdpPacket, b: UdpPacket) -> Self {
        let a_time = DateTime::from_timestamp_millis(a.timestamp).unwrap();
        let b_time = DateTime::from_timestamp_millis(b.timestamp).unwrap();
        Self {
            timestamp_avr: b_time + (a_time - b_time) / 2,
            a,
            b,
        }
    }
}
