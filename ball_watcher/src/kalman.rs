use std::{sync::Arc, time::Duration};

use nalgebra::{Matrix3, Matrix6, Matrix6x3, SMatrix, Vector3, Vector6};
use tokio::sync::RwLock;

use crate::{
    PairedFrames, State,
    data_association::ObjectTrack,
    protobuf::DetectedObject,
    types::{CameraId, ToVector3 as _},
};

const GRAVITY_ACCELERATION: f32 = 9.80665;

/// Tracks an object using `Kalman filter`. This would be created per an object.
pub struct ObjectTrackerKalman {
    obj_id: ObjectId,
    state: Arc<RwLock<State>>,
    input_mtx: Vector3<f32>,
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

impl ObjectTrackerKalman {
    pub fn new(obj_id: ObjectId, state: Arc<RwLock<State>>, coord: Vector3<f32>) -> Self {
        let f = {
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
        };
        let x_initial = Vector6::<f64>::zeros(); // TODO: use proper value

        /*let filter: Kalman1M<f32, 6, 3, 3, _, _> = Kalman1M::new_with_input(
            f,                   //6x6
            SMatrix::identity(), //6x6
            b,                   //6x3
            SMatrix::identity(), // TODO:use proper value
            SMatrix::identity(), //1x1
            x_initial,
        );*/
        Self {
            obj_id,
            input_mtx: Vector3::new(0., 0., -GRAVITY_ACCELERATION),
            state,
        }
    }

    pub fn obj_id(&self) -> ObjectId {
        self.obj_id
    }

    pub async fn update_and_check_collision(&mut self, pair: PairedFrames) {
        // camera location
        let cam_loc_a = self.get_camera_loc(pair.a.camera_id).await;
        let cam_loc_b = self.get_camera_loc(pair.b.camera_id).await;

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
    }

    /// Utility method to get camera location from [`State`][crate::State].
    fn get_camera_loc(&self, camera_id: impl Into<CameraId>) -> Vector3<f64> {
        self.state
            .read()
            .camera_locs
            .get(&camera_id.into())
            .unwrap()
            .to_vector3()
    }
}

impl ObjectTrack for ObjectTrackerKalman {
    fn evaluate_scores<'a>(
        &mut self,
        camera_id: impl Into<CameraId>,
        detections: impl Iterator<Item = &'a DetectedObject> + 'a,
    ) -> impl Iterator<Item = f64> + 'a {
        /*
        let prior_estimated =  self.filter.predict(self.input_mtx).unwrap();
        let estimated_coord = Vector3::new(prior_estimated.x, prior_estimated.y, prior_estimated.z);
        evaluate_scores_for_detections(
            detections,
            self.get_camera_loc(camera_id).await,
            estimated_coord,
        )*/
        todo!()
    }
}*/

/// Evaluates scores for each detections.
pub fn evaluate_scores_for_detections<'a>(
    detections: impl Iterator<Item = &'a DetectedObject>,
    camera_loc: Vector3<f64>,
    estimated_coord: Vector3<f64>,
) -> impl Iterator<Item = f64> {
    // TODO: minが一定距離より遠かったらNoneにする
    detections.map(move |obj| {
        // 点と直線の距離。TODO: 数式があってるか確認
        let lay = Vector3::new(obj.lay_x.into(), obj.lay_y.into(), obj.lay_z.into());
        let top = (estimated_coord - camera_loc).cross(&lay).norm();
        let bottom = lay.norm();
        (top / bottom).into()
    })
}

/// Zips two scores into one score.
/// (smaller is better)
fn zip_scores(score_a: f32, score_b: f32) -> f32 {
    // TODO: 調和平均とか取る
    score_a + score_b
}

#[cfg(test)]
mod tests {}
