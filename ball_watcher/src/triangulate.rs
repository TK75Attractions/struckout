use nalgebra::Vector3;

use crate::{protobuf::CameraLocation, types::Position3D};

#[must_use]
pub(crate) fn triangulate(
    camera_loc_1: CameraLocation,
    orientation_1: Vector3<f64>,
    camera_loc_2: CameraLocation,
    orientation_2: Vector3<f64>,
) -> Position3D {
    // TODO: From / Intoを使う
    let p: Vector3<f64> = camera_loc_1.into();
    let q: Vector3<f64> = camera_loc_2.into();
    let a = orientation_1;
    let b = orientation_2;

    let d = p - q;
    let n = a.cross(&b);

    let t = {
        let dividend = (d.cross(&b)).dot(&n);
        let divisor = n.dot(&n);
        dividend / divisor
    };
    let r = p + t * a;

    r.into()
}

#[cfg(test)]
mod tests {
    extern crate std;

    #[test]
    fn coordinate_is_correct() {
        todo!()
    }
}
