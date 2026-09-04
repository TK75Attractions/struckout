use std::collections::HashMap;
use tracing::warn;
use tracktor::assignment::{CostMatrix, hungarian};

use super::PairedFrames;
use crate::tracking::{ObjectTrack, TrackId};

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
    // rows are detections and columns are tracks
    let (costs_a, costs_b, idx_to_id) = create_cost_matrix(new_frame, tracks);

    let assignment_a = hungarian(&costs_a).unwrap();
    let assignment_b = hungarian(&costs_b).unwrap();

    // FIXME: aでinsert()しなかった場合get_mut().unwrap()でpanicしそう、テスト書く
    let mut ret = HashMap::new();
    for (det_idx, &track_idx) in assignment_a.mapping.iter().enumerate() {
        let Some(track_idx) = track_idx else {
            continue;
        };
        let id = idx_to_id.get(&track_idx).unwrap();
        ret.insert(*id, (Some(det_idx), None));
    }
    for (det_idx, &track_idx) in assignment_b.mapping.iter().enumerate() {
        let Some(track_idx) = track_idx else {
            warn!(det_idx, "hungarian assignment didn't match the column");
            continue;
        };
        let id = idx_to_id.get(&track_idx).unwrap();
        let dets = ret.get_mut(id).unwrap();
        dets.1 = Some(det_idx);
    }
    ret
}

/// rows are tracks (workers), columns are detections (jobs).
fn create_cost_matrix<T>(
    frame: &PairedFrames,
    tracks: &mut HashMap<TrackId, T>,
) -> (CostMatrix, CostMatrix, HashMap<usize, TrackId>)
where
    T: ObjectTrack,
{
    let mut idx_to_id = HashMap::new();

    let mut ret1 = CostMatrix::zeros(frame.a.detections.len(), tracks.len());
    let mut ret2 = CostMatrix::zeros(frame.a.detections.len(), tracks.len());

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
        for (i, &s) in scores_a.iter().enumerate() {
            ret1.set(track_idx, i, s);
        }
        for (i, &s) in scores_b.iter().enumerate() {
            ret2.set(track_idx, i, s);
        }
        idx_to_id.insert(track_idx, *track_id);
    }

    (ret1, ret2, idx_to_id)
}

#[cfg(test)]
mod tests {
    use std::ops::Range;

    use chrono::{DateTime, Utc};
    use nalgebra::Vector3;
    use rand::random_range;
    use struckout_proto::{Detection, DetectionsPacket};

    use crate::{
        tracking::{TrackIdGenerator, kalman::evaluate_scores_for_detections},
        types::CameraId,
    };

    use super::*;

    struct StubObjectTrack {
        scores_a: Vec<f64>,
        scores_b: Vec<f64>,
    }

    impl ObjectTrack for StubObjectTrack {
        fn evaluate_scores<'a>(
            &mut self,
            camera_id: impl Into<CameraId>,
            _detections: impl Iterator<Item = &'a Detection> + 'a,
            _timestamp: DateTime<Utc>,
        ) -> Vec<f64> {
            /*let ret = match camera_id.into() {
                id if id == CameraId::new(0) => self.scores_a.clone().into_iter(),
                id if id == CameraId::new(1) => self.scores_b.clone().into_iter(),
                _ => panic!("unknown camera!"),
            };
            ret*/
            Vec::new()
        }

        fn id(&self) -> crate::tracking::TrackId {
            todo!()
        }

        fn new(
            id: crate::tracking::TrackId,
            initial_position: Vector3<f64>,
            timestamp: DateTime<Utc>,
            camera_loc_provider: std::sync::Arc<crate::CameraLocationStore>,
        ) -> Self {
            todo!()
        }

        fn update_and_check_collision(
            &mut self,
            new_pos: crate::types::Position3D,
        ) -> Option<crate::types::CollisionPoint3D> {
            todo!()
        }
    }

    #[test]
    fn hungarian_works() {
        let costs_vec = vec![0.8, 0.3, 0.1, 0.2, 0.01, 0.9, 0.35, 0.6, 0.5];
        let costs = CostMatrix::from_vec(costs_vec.clone(), 3, 3).unwrap();
        let assignment = hungarian(&costs).unwrap();
        assert_eq!(costs_vec[assignment.mapping[0].unwrap()], 0.1);
        assert_eq!(costs_vec[3 + assignment.mapping[1].unwrap()], 0.01);
        assert_eq!(costs_vec[6 + assignment.mapping[2].unwrap()], 0.35);
    }

    const PREDICT_NOISE_RANGE: Range<f64> = -10.0..10.0;
    const MEASURE_NOISE_RANGE: Range<f64> = -10.0..10.0;

    fn frame_data(true_positions: Vec<Vector3<f64>>) -> PairedFrames {
        let frame_id = 5;
        PairedFrames {
            timestamp_avr: DateTime::default(),
            a: DetectionsPacket {
                camera_id: 0,
                session_id: "dummy".to_string(),
                timestamp: 0,
                frame_id,
                detections: true_positions
                    .iter()
                    .map(|p| measure_noised(*p))
                    .map(|p| Detection {
                        bbox_width: 0,
                        bbox_height: 0,
                        lay_x: p.x,
                        lay_y: p.y,
                        lay_z: p.z,
                    })
                    .collect(),
            },
            b: DetectionsPacket {
                camera_id: 1,
                session_id: "dummy".to_string(),
                timestamp: 0,
                frame_id,
                detections: true_positions
                    .iter()
                    .map(|p| measure_noised(*p))
                    .map(|p| Detection {
                        bbox_width: 0,
                        bbox_height: 0,
                        lay_x: p.x,
                        lay_y: p.y,
                        lay_z: p.z,
                    })
                    .collect(),
            },
        }
    }

    fn measure_noised(true_pos: Vector3<f64>) -> Vector3<f64> {
        let vec = true_pos
            .iter()
            .map(|x| x + random_range(MEASURE_NOISE_RANGE))
            .collect::<Vec<_>>();
        Vector3::from_vec(vec)
    }

    fn predict_noised(true_pos: Vector3<f64>) -> Vector3<f64> {
        let vec = true_pos
            .iter()
            .map(|x| x + random_range(PREDICT_NOISE_RANGE))
            .collect::<Vec<_>>();
        Vector3::from_vec(vec)
    }

    /// オブジェクトが3つ、detectionが3つありそれぞれがそれぞれに一対一対応している。重なり等もない単純なケース。
    fn associate_objects_works_in_simple_case() {
        let true_pos1 = Vector3::new(500., 100., 60.);
        let true_pos2 = Vector3::new(300., 130., 55.);
        let true_pos3 = Vector3::new(700., 50., 20.);

        let new_frame = frame_data(vec![true_pos1, true_pos2, true_pos3]);

        let predict1 = predict_noised(true_pos1);
        let predict2 = predict_noised(true_pos2);
        let predict3 = predict_noised(true_pos3);

        let camera_loc_a = Vector3::new(0., 0., 0.);
        let camera_loc_b = Vector3::new(0., 0., 0.);

        let track1 = StubObjectTrack {
            scores_a: evaluate_scores_for_detections(
                new_frame.a.detected_objects.iter(),
                camera_loc_a,
                predict1,
            ),
            scores_b: evaluate_scores_for_detections(
                new_frame.b.detected_objects.iter(),
                camera_loc_b,
                predict1,
            ),
        };
        let track2 = StubObjectTrack {
            scores_a: evaluate_scores_for_detections(
                new_frame.a.detected_objects.iter(),
                camera_loc_a,
                predict2,
            ),
            scores_b: evaluate_scores_for_detections(
                new_frame.b.detected_objects.iter(),
                camera_loc_b,
                predict2,
            ),
        };
        let track3 = StubObjectTrack {
            scores_a: evaluate_scores_for_detections(
                new_frame.a.detected_objects.iter(),
                camera_loc_a,
                predict3,
            ),
            scores_b: evaluate_scores_for_detections(
                new_frame.b.detected_objects.iter(),
                camera_loc_b,
                predict3,
            ),
        };
        let id_gen = TrackIdGenerator::new();
        let id_1 = id_gen.next();
        let id_2 = id_gen.next();
        let id_3 = id_gen.next();
        let mut tracks = vec![(id_1, track1), (id_2, track2), (id_3, track3)].into();

        let assignment = associate_objects(&mut tracks, &new_frame);
        assert_eq!(assignment.len(), 3);
        assert_eq!(assignment[&0], (Some(0), Some(0)));
        assert_eq!(assignment[&1], (Some(1), Some(1)));
        assert_eq!(assignment[&2], (Some(2), Some(2)));
    }
}
