use std::collections::{HashMap, VecDeque};

use chrono::{DateTime, TimeDelta, Utc};
use serde::Serialize;
use struckout_proto::DetectionsPacket;
use tokio::sync::mpsc;

mod db;
mod network;
pub use network::{NetworkDetectionInput, NetworkDetectionInputCreationError};
use tracing::{trace, warn};

#[cfg(feature = "input-sqlite")]
mod sqlite;
#[cfg(feature = "input-sqlite")]
pub use sqlite::SqliteDetectionInput;

pub trait DetectionInput {
    fn start(
        self,
        tx: mpsc::Sender<PairedFrames>,
    ) -> impl std::future::Future<Output = std::io::Result<()>> + Send;
}

const FRAME_MATCHING_DELTA: TimeDelta = TimeDelta::milliseconds(20);
const FRAME_BUFFER_DURATION: TimeDelta = TimeDelta::milliseconds(100);

struct BufferedFrame {
    aligned_timestamp: DateTime<Utc>,
    packet: DetectionsPacket,
}

pub struct FramePairMatcher {
    frames: VecDeque<BufferedFrame>,
    clock_offsets: HashMap<(u32, String), TimeDelta>,
    latest_timestamp: Option<DateTime<Utc>>,
}

impl FramePairMatcher {
    fn new() -> Self {
        Self {
            frames: VecDeque::new(),
            clock_offsets: HashMap::new(),
            latest_timestamp: None,
        }
    }

    fn pair_frame(
        &mut self,
        received_at: DateTime<Utc>,
        packet: DetectionsPacket,
    ) -> Option<PairedFrames> {
        let aligned_timestamp = self.align_timestamp(received_at, &packet)?;
        let cur_frame_cam_id = packet.camera_id;
        self.latest_timestamp = Some(
            self.latest_timestamp
                .map_or(aligned_timestamp, |latest| latest.max(aligned_timestamp)),
        );

        let matching_idx = self
            .frames
            .iter()
            .enumerate()
            .filter(|(_, candidate)| candidate.packet.camera_id != cur_frame_cam_id)
            .filter_map(|(idx, candidate)| {
                let delta = (candidate.aligned_timestamp - aligned_timestamp).abs();
                (delta <= FRAME_MATCHING_DELTA).then_some((idx, delta))
            })
            .min_by_key(|(_, delta)| *delta)
            .map(|(idx, _)| idx);

        if let Some(idx) = matching_idx {
            let matched = self.frames.remove(idx).unwrap();
            trace!(time = ?aligned_timestamp, "created pair");
            return Some(PairedFrames::new(
                packet,
                aligned_timestamp,
                matched.packet,
                matched.aligned_timestamp,
            ));
        }

        self.frames.push_back(BufferedFrame {
            aligned_timestamp,
            packet,
        });
        let oldest_timestamp = self.latest_timestamp.unwrap() - FRAME_BUFFER_DURATION;
        self.frames
            .retain(|frame| frame.aligned_timestamp >= oldest_timestamp);
        None
    }

    fn align_timestamp(
        &mut self,
        received_at: DateTime<Utc>,
        packet: &DetectionsPacket,
    ) -> Option<DateTime<Utc>> {
        let Some(camera_timestamp) = DateTime::from_timestamp_millis(packet.timestamp) else {
            warn!(
                timestamp = packet.timestamp,
                "received an invalid frame timestamp"
            );
            return None;
        };
        let observed_offset = received_at - camera_timestamp;
        let offset = self
            .clock_offsets
            .entry((packet.camera_id, packet.session_id.clone()))
            .and_modify(|offset| *offset = (*offset).min(observed_offset))
            .or_insert(observed_offset);
        Some(camera_timestamp + *offset)
    }
}

/// Paired frames from two cameras at the same timestamp.
#[derive(Debug, Serialize)]
pub struct PairedFrames {
    pub timestamp_avr: DateTime<Utc>,
    pub a: DetectionsPacket,
    pub b: DetectionsPacket,
}

impl PairedFrames {
    fn new(
        a: DetectionsPacket,
        a_time: DateTime<Utc>,
        b: DetectionsPacket,
        b_time: DateTime<Utc>,
    ) -> Self {
        let (a, a_time, b, b_time) = if a.camera_id <= b.camera_id {
            (a, a_time, b, b_time)
        } else {
            (b, b_time, a, a_time)
        };
        Self {
            timestamp_avr: b_time + (a_time - b_time) / 2,
            a,
            b,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packet(camera_id: u32, timestamp: i64, frame_id: u64) -> DetectionsPacket {
        DetectionsPacket {
            camera_id,
            session_id: format!("camera-{camera_id}"),
            timestamp,
            frame_id,
            detections: Vec::new(),
        }
    }

    fn time(timestamp: i64) -> DateTime<Utc> {
        DateTime::from_timestamp_millis(timestamp).unwrap()
    }

    #[test]
    fn pairs_frames_using_millisecond_timestamps() {
        let mut matcher = FramePairMatcher::new();

        assert!(
            matcher
                .pair_frame(time(10_000), packet(0, 1_790_042_290_000, 1))
                .is_none()
        );
        let pair = matcher
            .pair_frame(time(10_012), packet(1, 1_790_042_290_012, 2))
            .unwrap();

        assert_eq!(pair.a.camera_id, 0);
        assert_eq!(pair.b.camera_id, 1);
        assert_eq!(pair.a.frame_id, 1);
        assert_eq!(pair.b.frame_id, 2);
    }

    #[test]
    fn compensates_for_camera_clock_offsets() {
        let mut matcher = FramePairMatcher::new();

        matcher.pair_frame(time(10_000), packet(0, 1_000, 1));
        let pair = matcher
            .pair_frame(time(10_012), packet(1, 2_200, 2))
            .unwrap();

        assert_eq!(pair.a.frame_id, 1);
        assert_eq!(pair.b.frame_id, 2);
        assert_eq!(pair.timestamp_avr, time(10_006));
    }

    #[test]
    fn picks_the_closest_frame_from_the_other_camera() {
        let mut matcher = FramePairMatcher::new();

        matcher.pair_frame(time(2_000), packet(0, 1_000, 1));
        matcher.pair_frame(time(2_015), packet(0, 1_015, 2));
        let pair = matcher
            .pair_frame(time(2_012), packet(1, 5_000, 3))
            .unwrap();

        assert_eq!(pair.a.frame_id, 2);
        assert_eq!(pair.b.frame_id, 3);
    }

    #[test]
    fn does_not_pair_frames_outside_the_matching_window() {
        let mut matcher = FramePairMatcher::new();

        matcher.pair_frame(time(2_000), packet(0, 1_000, 1));
        assert!(
            matcher
                .pair_frame(time(2_021), packet(1, 5_000, 2))
                .is_none()
        );
    }

    #[test]
    fn discards_stale_unmatched_frames() {
        let mut matcher = FramePairMatcher::new();

        matcher.pair_frame(time(2_000), packet(0, 1_000, 1));
        matcher.pair_frame(time(2_101), packet(0, 1_101, 2));
        assert_eq!(matcher.frames.len(), 1);
        assert_eq!(matcher.frames.front().unwrap().packet.frame_id, 2);

        let pair = matcher
            .pair_frame(time(2_105), packet(1, 5_000, 3))
            .unwrap();
        assert_eq!(pair.a.frame_id, 2);
        assert_eq!(pair.b.frame_id, 3);
    }
}
