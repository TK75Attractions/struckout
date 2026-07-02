use std::collections::HashMap;

use chrono::{DateTime, Utc};
use struckout_proto::DetectedObject;
use tracing::warn;
use tracktor::assignment::{CostMatrix, hungarian};

use super::PairedFrames;
use crate::types::CameraId;

/// Tracks an object.
pub trait ObjectTrack {
    /// Predict object location and evaluate scores for each detections.
    fn evaluate_scores<'a>(
        &mut self,
        camera_id: impl Into<CameraId>,
        detections: impl Iterator<Item = &'a DetectedObject> + Clone + 'a,
        timestamp: DateTime<Utc>,
    ) -> impl Iterator<Item = f64> + Clone + 'a;
}

/// Associates detections to known objects (trackers).
///
/// Returns `tracker_idx` -> (`detection_idx_a`, `detection_idx_b`).
/// detection_idx will be `None` if detection is likely new object.
pub fn associate_objects<T>(
    tracks: &mut Vec<T>,
    new_frame: &PairedFrames,
) -> HashMap<usize, (Option<usize>, Option<usize>)>
where
    T: ObjectTrack,
{
    // rows are detections and columns are tracks
    let (costs_a, costs_b) = {
        let mut ret1 = CostMatrix::zeros(new_frame.a.detected_objects.len(), tracks.len());
        let mut ret2 = CostMatrix::zeros(new_frame.a.detected_objects.len(), tracks.len());

        for (obj_idx, obj) in tracks.iter_mut().enumerate() {
            let scores_a = obj.evaluate_scores(
                new_frame.a.camera_id,
                new_frame.a.detected_objects.iter(),
                new_frame.timestamp_avr,
            );
            let scores_b = obj.evaluate_scores(
                new_frame.b.camera_id,
                new_frame.b.detected_objects.iter(),
                new_frame.timestamp_avr,
            );
            for (i, s) in scores_a.enumerate() {
                ret1.set(i, obj_idx, s);
            }
            for (i, s) in scores_b.enumerate() {
                ret2.set(i, obj_idx, s);
            }
        }

        (ret1, ret2)
    };

    let assignment_a = hungarian(&costs_a).unwrap();
    let assignment_b = hungarian(&costs_b).unwrap();

    // FIXME: aでinsert()しなかった場合get_mut().unwrap()でpanicしそう、テスト書く
    let mut ret = HashMap::new();
    for (det_idx, track_idx) in assignment_a.mapping.iter().enumerate() {
        let Some(track_idx) = track_idx else {
            continue;
        };
        ret.insert(*track_idx, (Some(det_idx), None));
    }
    for (det_idx, track_idx) in assignment_b.mapping.iter().enumerate() {
        let Some(track_idx) = track_idx else {
            warn!(det_idx, "hungarian assignment didn't match the column");
            continue;
        };
        let dets = ret.get_mut(track_idx).unwrap();
        dets.1 = Some(det_idx);
    }
    ret
}

#[cfg(test)]
mod tests {
    use std::ops::Range;

    use chrono::DateTime;
    use nalgebra::Vector3;
    use rand::random_range;

    use crate::{kalman::evaluate_scores_for_detections, protobuf::UdpPacket};

    use super::*;

    struct StubObjectTrack {
        scores_a: Vec<f64>,
        scores_b: Vec<f64>,
    }

    impl ObjectTrack for StubObjectTrack {
        async fn evaluate_scores<'a>(
            &mut self,
            camera_id: impl Into<CameraId>,
            _detections: impl Iterator<Item = &'a DetectedObject> + 'a,
        ) -> impl Iterator<Item = f64> + Clone + 'a {
            let ret = match camera_id.into() {
                id if id == CameraId::new(0) => self.scores_a.clone().into_iter(),
                id if id == CameraId::new(1) => self.scores_b.clone().into_iter(),
                _ => panic!("unknown camera!"),
            };
            ret
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
            a: UdpPacket {
                camera_id: 0,
                timestamp: 0,
                frame_id,
                detected_objects: true_positions
                    .iter()
                    .map(|p| measure_noised(*p))
                    .map(|p| DetectedObject {
                        bbox_width: 0,
                        bbox_height: 0,
                        lay_x: p.x,
                        lay_y: p.y,
                        lay_z: p.z,
                    })
                    .collect(),
            },
            b: UdpPacket {
                camera_id: 1,
                timestamp: 0,
                frame_id,
                detected_objects: true_positions
                    .iter()
                    .map(|p| measure_noised(*p))
                    .map(|p| DetectedObject {
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
            )
            .collect(),
            scores_b: evaluate_scores_for_detections(
                new_frame.b.detected_objects.iter(),
                camera_loc_b,
                predict1,
            )
            .collect(),
        };
        let track2 = StubObjectTrack {
            scores_a: evaluate_scores_for_detections(
                new_frame.a.detected_objects.iter(),
                camera_loc_a,
                predict2,
            )
            .collect(),
            scores_b: evaluate_scores_for_detections(
                new_frame.b.detected_objects.iter(),
                camera_loc_b,
                predict2,
            )
            .collect(),
        };
        let track3 = StubObjectTrack {
            scores_a: evaluate_scores_for_detections(
                new_frame.a.detected_objects.iter(),
                camera_loc_a,
                predict3,
            )
            .collect(),
            scores_b: evaluate_scores_for_detections(
                new_frame.b.detected_objects.iter(),
                camera_loc_b,
                predict3,
            )
            .collect(),
        };
        let mut tracks = vec![track1, track2, track3];

        let assignment = associate_objects(&mut tracks, &new_frame);
        assert_eq!(assignment.len(), 3);
        assert_eq!(assignment[&0], (Some(0), Some(0)));
        assert_eq!(assignment[&1], (Some(1), Some(1)));
        assert_eq!(assignment[&2], (Some(2), Some(2)));
    }
}
