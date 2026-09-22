use nalgebra::Vector3;
use struckout_proto::CameraLocation;

use crate::types::{Position3D, ToVector3};

#[derive(Debug, Clone, Copy)]
pub struct Triangulation {
    pub position: Position3D,
    pub ray_distance: f64,
}

#[must_use]
pub fn triangulate(
    camera_loc_1: CameraLocation,
    orientation_1: Vector3<f64>,
    camera_loc_2: CameraLocation,
    orientation_2: Vector3<f64>,
) -> Option<Triangulation> {
    let p = camera_loc_1.to_vector3();
    let q = camera_loc_2.to_vector3();
    let a = orientation_1;
    let b = orientation_2;

    let a_dot_a = a.dot(&a);
    let a_dot_b = a.dot(&b);
    let b_dot_b = b.dot(&b);
    let p_to_q = p - q;
    let a_dot_p_to_q = a.dot(&p_to_q);
    let b_dot_p_to_q = b.dot(&p_to_q);
    let denominator = a_dot_a * b_dot_b - a_dot_b * a_dot_b;

    if denominator.abs() < f64::EPSILON {
        return None;
    }

    let t = (a_dot_b * b_dot_p_to_q - b_dot_b * a_dot_p_to_q) / denominator;
    let s = (a_dot_a * b_dot_p_to_q - a_dot_b * a_dot_p_to_q) / denominator;
    if t < 0.0 || s < 0.0 {
        return None;
    }

    let point_a = p + t * a;
    let point_b = q + s * b;
    Some(Triangulation {
        position: ((point_a + point_b) / 2.0).into(),
        ray_distance: (point_a - point_b).norm(),
    })
}

#[cfg(test)]
mod tests {
    use approx::assert_relative_eq;

    use super::*;

    fn camera(x: f64, y: f64, z: f64) -> CameraLocation {
        CameraLocation { x, y, z }
    }

    #[test]
    fn returns_the_intersection_of_two_rays() {
        let result = triangulate(
            camera(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            camera(1.0, -1.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
        )
        .unwrap();

        assert_relative_eq!(result.position.x, 1.0);
        assert_relative_eq!(result.position.y, 0.0);
        assert_relative_eq!(result.position.z, 0.0);
        assert_relative_eq!(result.ray_distance, 0.0);
    }

    #[test]
    fn returns_the_midpoint_for_skew_rays() {
        let result = triangulate(
            camera(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            camera(1.0, -1.0, 2.0),
            Vector3::new(0.0, 1.0, 0.0),
        )
        .unwrap();

        assert_relative_eq!(result.position.x, 1.0);
        assert_relative_eq!(result.position.y, 0.0);
        assert_relative_eq!(result.position.z, 1.0);
        assert_relative_eq!(result.ray_distance, 2.0);
    }

    #[test]
    fn rejects_parallel_rays() {
        let result = triangulate(
            camera(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            camera(0.0, 1.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
        );

        assert!(result.is_none());
    }

    #[test]
    fn rejects_intersections_behind_a_camera() {
        let result = triangulate(
            camera(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            camera(-1.0, -1.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
        );

        assert!(result.is_none());
    }
}
