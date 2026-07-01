use chrono::{DateTime, Utc};
use struckout_proto::UdpPacket;

use crate::types::CollisionPoint3D;

mod data_association;
mod kalman;
mod triangulate;

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
