use std::{sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use nalgebra::Vector3;
use struckout_proto::Detection;
use tracktor::{
    filters::kalman::KalmanFilter,
    models::{ConstantVelocity3D, PositionSensor3D},
    prelude::*,
    types::spaces::{StateCovariance, StateVector},
};

use crate::{
    CameraLocationStore,
    tracking::{ObjectTrack, TrackId},
    types::{CameraId, CollisionPoint3D, Position3D, ToVector3},
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
        evaluate_scores_for_detections(
            detections,
            self.camera_locs.get(camera_id.into()).unwrap().to_vector3(),
            estimated_coord,
        )
    }

    fn update_and_check_collision(&mut self, new_pos: Position3D) -> Option<CollisionPoint3D> {
        let measurement = Vector::from_svector(new_pos.to_vector3());
        let estimate = self
            .filter
            .update(&self.kalman_state, &measurement)
            .unwrap(); // FIXME: たぶんunwrapしないほうがいい
        if estimate.mean.get(0).copied().unwrap() <= 0. {
            Some(CollisionPoint3D {
                x: 0., // FIXME: ちゃんと計算する
                y: estimate.mean.get(1).copied().unwrap(),
                z: estimate.mean.get(2).copied().unwrap(),
            })
        } else {
            None
        }
    }
}

/// Evaluates scores for each detections.
pub fn evaluate_scores_for_detections<'a>(
    detections: impl Iterator<Item = &'a Detection> + Clone,
    camera_loc: Vector3<f64>,
    estimated_coord: Vector3<f64>,
) -> Vec<f64> {
    // TODO: minが一定距離より遠かったらNoneにする
    detections
        .map(move |obj| {
            // 点と直線の距離。TODO: 数式があってるか確認
            let lay = Vector3::new(obj.lay_x.into(), obj.lay_y.into(), obj.lay_z.into());
            let top = (estimated_coord - camera_loc).cross(&lay).norm();
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
mod tests {}
