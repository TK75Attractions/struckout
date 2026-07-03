use std::time::Duration;

use chrono::{DateTime, Utc};
use nalgebra::Vector3;
use struckout_proto::DetectedObject;
use tracktor::{
    filters::kalman::KalmanFilter,
    models::{ConstantVelocity3D, PositionSensor3D},
    prelude::*,
    types::spaces::{StateCovariance, StateVector},
};

use crate::{
    tracking::{CameraLocationProvider, ObjectTrack},
    types::{CameraId, CollisionPoint3D, Position3D, ToVector3},
};

const GRAVITY_ACCELERATION: f32 = 9.80665;

type TheKalmanFilter = KalmanFilter<f64, ConstantVelocity3D<f64>, PositionSensor3D<f64>, 6, 3>;

/// Tracks an object using `Kalman filter`. This would be created per an object.
pub struct KalmanTrack<P> {
    obj_id: ObjectId,
    input_mtx: Vector3<f32>,
    filter: TheKalmanFilter,
    kalman_state: KalmanState<f64, 6>,
    prev_timestamp: DateTime<Utc>,
    camera_loc_provider: P,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectId(usize);

impl ObjectId {
    #[must_use]
    pub fn next(&self) -> Self {
        Self(self.0 + 1)
    }
}

const DELTA_T: f32 = Duration::from_millis(16).as_secs_f32();

impl<P> KalmanTrack<P>
where
    P: CameraLocationProvider,
{
    pub fn new(
        obj_id: ObjectId,
        initial_position: Vector3<f64>,
        timestamp: DateTime<Utc>,
        camera_loc_provider: P,
    ) -> Self {
        /*let f = {
            let identity = Matrix3::<f32>::identity();
            let delta_t = identity * DELTA_T;
            let zeros = Matrix3::<f32>::zeros();

            let mut ret = Matrix6::<f32>::zeros();
            ret.fixed_view_mut::<3, 3>(0, 0).copy_from(&identity);
            ret.fixed_view_mut::<3, 3>(3, 0).copy_from(&zeros);
            ret.fixed_view_mut::<3, 3>(0, 3).copy_from(&delta_t);
            ret.fixed_view_mut::<3, 3>(3, 3).copy_from(&identity);
            ret
        };
        let b = {
            let identity = Matrix3::<f32>::identity();
            let top = ((DELTA_T * DELTA_T) / 2.) * identity;
            let bottom = DELTA_T * identity;
            let mut ret = Matrix6x3::zeros();
            ret.fixed_view_mut::<3, 3>(0, 0).copy_from(&top);
            ret.fixed_view_mut::<3, 3>(3, 0).copy_from(&bottom);
            ret
        };*/

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
            obj_id,
            input_mtx: Vector3::new(0., 0., -GRAVITY_ACCELERATION),
            filter,
            kalman_state,
            prev_timestamp: timestamp,
            camera_loc_provider,
        }
    }

    pub fn obj_id(&self) -> ObjectId {
        self.obj_id
    }
}

impl<P> ObjectTrack for KalmanTrack<P>
where
    P: CameraLocationProvider,
{
    fn evaluate_scores<'a>(
        &mut self,
        camera_id: impl Into<CameraId>,
        detections: impl Iterator<Item = &'a DetectedObject> + Clone + 'a,
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
            self.camera_loc_provider
                .get(camera_id.into())
                .unwrap()
                .to_vector3(),
            estimated_coord,
        )
    }

    fn update_and_check_collision(&mut self, new_pos: Position3D) -> Option<CollisionPoint3D> {
        //task::spawn_blocking(|| {

        /*let Some(idx_a) =
            evaluate_scores_for_objects(pair.a.detected_objects.iter(), cam_loc_a, estimated_coord)
        else {
            // there was no object in this frame
            return;
        };

        let Some(idx_b) =
            evaluate_scores_for_objects(pair.b.detected_objects.iter(), cam_loc_b, estimated_coord)
        else {
            warn!(
                camera_id_a = pair.a.camera_id,
                camera_id_b = pair.b.camera_id,
                timestamp = ?pair.timestamp_avr,
                "there was some object in the frame from `camera A` but it's not in `camera B`"
            );
            return;
        };*/

        // `idx` must exist because it comes from `find_corresponding_object()`
        //let lay_a = pair.a.detected_objects.get(idx_a).unwrap().get_lay();
        //let lay_b = pair.b.detected_objects.get(idx_b).unwrap().get_lay();

        //let measured_coord = triangulate(cam_loc_a, lay_a, cam_loc_b, lay_b).to_vector3();

        //self.filter.update(measured_coord).unwrap();
        //})
        //.await
        //.unwrap();
        todo!()
    }
}

/// Evaluates scores for each detections.
pub fn evaluate_scores_for_detections<'a>(
    detections: impl Iterator<Item = &'a DetectedObject> + Clone,
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
