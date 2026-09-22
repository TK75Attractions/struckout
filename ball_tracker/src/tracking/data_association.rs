use std::collections::HashMap;

use tracktor::assignment::{CostMatrix, hungarian_gated};

use super::PairedFrames;
use crate::tracking::{ObjectTrack, TrackId};

const TRACK_ASSOCIATION_GATE: f64 = 100.0;

/// Associates detections to known objects (trackers).
///
/// Returns `tracker_idx` -> (`detection_idx_a`, `detection_idx_b`).
/// detection_idx will be `None` if detection is likely new object.
pub fn associate_objects<T>(
    tracks: &mut HashMap<TrackId, T>,
    new_frame: &PairedFrames,
) -> HashMap<TrackId, (Option<usize>, Option<usize>)>
where
    T: ObjectTrack,
{
    let (costs_a, costs_b, idx_to_id) = create_cost_matrix(new_frame, tracks);

    let assignment_a = hungarian_gated(&costs_a, TRACK_ASSOCIATION_GATE).unwrap();
    let assignment_b = hungarian_gated(&costs_b, TRACK_ASSOCIATION_GATE).unwrap();

    idx_to_id
        .into_iter()
        .enumerate()
        .map(|(track_idx, track_id)| {
            let detection_a = assignment_a.mapping.get(track_idx).copied().flatten();
            let detection_b = assignment_b.mapping.get(track_idx).copied().flatten();
            (track_id, (detection_a, detection_b))
        })
        .collect()
}

fn create_cost_matrix<T>(
    frame: &PairedFrames,
    tracks: &mut HashMap<TrackId, T>,
) -> (CostMatrix, CostMatrix, Vec<TrackId>)
where
    T: ObjectTrack,
{
    let mut idx_to_id = Vec::with_capacity(tracks.len());

    let mut ret1 = CostMatrix::zeros(tracks.len(), frame.a.detections.len());
    let mut ret2 = CostMatrix::zeros(tracks.len(), frame.b.detections.len());

    for (track_idx, (track_id, track)) in tracks.iter_mut().enumerate() {
        let scores_a = track.evaluate_scores(
            frame.a.camera_id,
            frame.a.detections.iter(),
            frame.timestamp_avr,
        );
        let scores_b = track.evaluate_scores(
            frame.b.camera_id,
            frame.b.detections.iter(),
            frame.timestamp_avr,
        );
        for (detection_idx, &score) in scores_a.iter().enumerate() {
            ret1.set(track_idx, detection_idx, score);
        }
        for (detection_idx, &score) in scores_b.iter().enumerate() {
            ret2.set(track_idx, detection_idx, score);
        }
        idx_to_id.push(*track_id);
    }

    (ret1, ret2, idx_to_id)
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};
    use nalgebra::Vector3;
    use struckout_proto::{Detection, DetectionsPacket};

    use crate::{tracking::TrackIdGenerator, types::CameraId};

    use super::*;

    struct StubObjectTrack {
        id: TrackId,
        scores_a: Vec<f64>,
        scores_b: Vec<f64>,
    }

    impl ObjectTrack for StubObjectTrack {
        fn evaluate_scores<'a>(
            &mut self,
            camera_id: impl Into<CameraId>,
            _detections: impl Iterator<Item = &'a Detection> + Clone + 'a,
            _timestamp: DateTime<Utc>,
        ) -> Vec<f64> {
            match camera_id.into() {
                id if id == CameraId::new(0) => self.scores_a.clone(),
                id if id == CameraId::new(1) => self.scores_b.clone(),
                _ => panic!("unknown camera"),
            }
        }

        fn id(&self) -> TrackId {
            self.id
        }

        fn new(
            _id: TrackId,
            _initial_position: Vector3<f64>,
            _timestamp: DateTime<Utc>,
            _camera_loc_provider: std::sync::Arc<crate::CameraLocationStore>,
        ) -> Self {
            unreachable!()
        }

        fn update_and_check_collision(
            &mut self,
            _new_pos: crate::types::Position3D,
        ) -> Option<crate::types::CollisionPoint3D> {
            None
        }
    }

    fn frame_data(detections_a: usize, detections_b: usize) -> PairedFrames {
        fn detections(count: usize) -> Vec<Detection> {
            (0..count)
                .map(|_| Detection {
                    bbox_width: 10,
                    bbox_height: 10,
                    lay_x: 0.0,
                    lay_y: 0.0,
                    lay_z: 1.0,
                })
                .collect()
        }

        PairedFrames {
            timestamp_avr: DateTime::default(),
            a: DetectionsPacket {
                camera_id: 0,
                session_id: "dummy-a".to_string(),
                timestamp: 0,
                frame_id: 1,
                detections: detections(detections_a),
            },
            b: DetectionsPacket {
                camera_id: 1,
                session_id: "dummy-b".to_string(),
                timestamp: 0,
                frame_id: 1,
                detections: detections(detections_b),
            },
        }
    }

    #[test]
    fn associates_each_track_independently_for_both_cameras() {
        let new_frame = frame_data(2, 2);
        let id_gen = TrackIdGenerator::new();
        let id_1 = id_gen.next();
        let id_2 = id_gen.next();
        let mut tracks = [
            (
                id_1,
                StubObjectTrack {
                    id: id_1,
                    scores_a: vec![0.1, 10.0],
                    scores_b: vec![10.0, 0.1],
                },
            ),
            (
                id_2,
                StubObjectTrack {
                    id: id_2,
                    scores_a: vec![10.0, 0.1],
                    scores_b: vec![0.1, 10.0],
                },
            ),
        ]
        .into();

        let assignment = associate_objects(&mut tracks, &new_frame);
        assert_eq!(assignment[&id_1], (Some(0), Some(1)));
        assert_eq!(assignment[&id_2], (Some(1), Some(0)));
    }

    #[test]
    fn supports_different_detection_counts_between_cameras() {
        let new_frame = frame_data(1, 2);
        let id = TrackIdGenerator::new().next();
        let mut tracks = [(
            id,
            StubObjectTrack {
                id,
                scores_a: vec![1.0],
                scores_b: vec![5.0, 1.0],
            },
        )]
        .into();

        let assignment = associate_objects(&mut tracks, &new_frame);
        assert_eq!(assignment[&id], (Some(0), Some(1)));
    }

    #[test]
    fn rejects_detections_outside_the_gate() {
        let new_frame = frame_data(1, 1);
        let id = TrackIdGenerator::new().next();
        let mut tracks = [(
            id,
            StubObjectTrack {
                id,
                scores_a: vec![TRACK_ASSOCIATION_GATE + 1.0],
                scores_b: vec![TRACK_ASSOCIATION_GATE + 1.0],
            },
        )]
        .into();

        let assignment = associate_objects(&mut tracks, &new_frame);
        assert_eq!(assignment[&id], (None, None));
    }
}
