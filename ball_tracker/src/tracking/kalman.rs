use std::{sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use nalgebra::Vector3;
use struckout_proto::{CameraPose, Detection};
use tracktor::{
    filters::kalman::KalmanFilter,
    models::{ConstantVelocity3D, PositionSensor3D},
    prelude::*,
    types::spaces::{StateCovariance, StateVector},
};

use crate::{
    CameraLocationStore,
    tracking::{ObjectTrack, TrackId},
    types::{
        CameraId, CollisionPoint3D, GetLayFromDetection as _, Position3D, RotateToWorld as _,
        ToVector3,
    },
};

const GRAVITY_ACCELERATION: f32 = 9.80665;

type TheKalmanFilter = KalmanFilter<f64, ConstantVelocity3D<f64>, PositionSensor3D<f64>, 6, 3>;

/// Tracks an object using `Kalman filter`. This would be created per an object.
pub struct KalmanTrack {
    id: TrackId,
    input_mtx: Vector3<f32>,
    filter: TheKalmanFilter,
    kalman_state: KalmanState<f64, 6>,
    prev_timestamp: DateTime<Utc>,
    camera_locs: Arc<CameraLocationStore>,
}

const DELTA_T: f32 = Duration::from_millis(16).as_secs_f32();

impl ObjectTrack for KalmanTrack {
    fn new(
        id: TrackId,
        initial_position: Vector3<f64>,
        timestamp: DateTime<Utc>,
        camera_loc_provider: Arc<CameraLocationStore>,
    ) -> Self {
        // TODO: set proper value
        let transition = ConstantVelocity3D::new(1.0, 0.99);
        let sensor = PositionSensor3D::new(5.0, 0.95);
        let filter = KalmanFilter::new(transition, sensor);

        let initial_velocity = Vector3::new(5.0, 5.0, 5.0);
        let initial_state = StateVector::from_array([
            initial_position[0],
            initial_position[1],
            initial_position[2],
            initial_velocity[0],
            initial_velocity[1],
            initial_velocity[2],
        ]);
        let initial_cov =
            StateCovariance::from_diagonal(&nalgebra::vector![10.0, 10.0, 1.0, 1.0, 0.5, 0.5]);
        let kalman_state = KalmanState::new(initial_state, initial_cov);

        Self {
            id,
            input_mtx: Vector3::new(0., 0., -GRAVITY_ACCELERATION),
            filter,
            kalman_state,
            prev_timestamp: timestamp,
            camera_locs: camera_loc_provider,
        }
    }

    fn id(&self) -> TrackId {
        self.id
    }

    fn evaluate_scores<'a>(
        &mut self,
        camera_id: impl Into<CameraId>,
        detections: impl Iterator<Item = &'a Detection> + Clone + 'a,
        timestamp: DateTime<Utc>,
    ) -> Vec<f64> {
        let state = self.filter.predict(
            &self.kalman_state,
            (timestamp - self.prev_timestamp).as_seconds_f64(),
        );
        self.prev_timestamp = timestamp;
        let estimated_coord = Vector3::from([
            state.mean.get(0).unwrap().to_owned(),
            state.mean.get(1).unwrap().to_owned(),
            state.mean.get(2).unwrap().to_owned(),
        ]);
        self.kalman_state = state;
        let camera_location = self.camera_locs.get(camera_id.into()).unwrap();
        evaluate_scores_for_detections(detections, camera_location, estimated_coord)
    }

    fn update_and_check_collision(&mut self, new_pos: Position3D) -> Option<CollisionPoint3D> {
        let measurement = Vector::from_svector(new_pos.to_vector3());
        let estimate = self
            .filter
            .update(&self.kalman_state, &measurement)
            .unwrap(); // FIXME: たぶんunwrapしないほうがいい
        let collision = if estimate.mean.get(0).copied().unwrap() <= 0. {
            Some(CollisionPoint3D {
                x: 0., // FIXME: ちゃんと計算する
                y: estimate.mean.get(1).copied().unwrap(),
                z: estimate.mean.get(2).copied().unwrap(),
            })
        } else {
            None
        };
        self.kalman_state = estimate;
        collision
    }
}

/// Evaluates scores for each detections.
pub fn evaluate_scores_for_detections<'a>(
    detections: impl Iterator<Item = &'a Detection> + Clone,
    camera_pose: CameraPose,
    estimated_coord: Vector3<f64>,
) -> Vec<f64> {
    // TODO: minが一定距離より遠かったらNoneにする
    let camera_position = camera_pose.to_vector3();
    detections
        .map(move |obj| {
            // 点と直線の距離。TODO: 数式があってるか確認
            let lay = camera_pose.rotate_to_world(obj.get_lay());
            let top = (estimated_coord - camera_position).cross(&lay).norm();
            let bottom = lay.norm();
            (top / bottom).into()
        })
        .collect()
}

/// Zips two scores into one score.
/// (smaller is better)
fn zip_scores(score_a: f32, score_b: f32) -> f32 {
    // TODO: 調和平均とか取る
    score_a + score_b
}

#[cfg(test)]
mod tests {
    use approx::assert_relative_eq;

    use super::*;

    #[test]
    fn scores_detection_after_rotating_its_ray_into_world_coordinates() {
        let camera_location = CameraPose {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            rotation_x_degrees: 0.0,
            rotation_y_degrees: 0.0,
            rotation_z_degrees: 90.0,
        };
        let detection = Detection {
            bbox_width: 10,
            bbox_height: 10,
            lay_x: 1.0,
            lay_y: 0.0,
            lay_z: 0.0,
        };

        let scores = evaluate_scores_for_detections(
            std::iter::once(&detection),
            camera_location,
            Vector3::new(0.0, 100.0, 0.0),
        );

        assert_relative_eq!(scores[0], 0.0, epsilon = 1e-12);
    }
}
