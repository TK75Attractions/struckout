use nalgebra::Vector3;

use crate::{CameraLocation, Frame};

pub(crate) fn calc_coordinate(
    camera_loc_1: CameraLocation,
    orientation_1: Orientation,
    camera_loc_2: CameraLocation,
    orientation_2: Orientation,
) -> Coordinate {
    // TODO: From / Intoを使う
    let p = camera_loc_1.into_vector3();
    let q = camera_loc_2.into_vector3();
    let a = orientation_1.into_vector3();
    let b = orientation_2.into_vector3();

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

pub struct Orientation {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl From<Frame> for Orientation {
    fn from(value: Frame) -> Self {
        Self {
            x: value.x,
            y: value.y,
            z: value.z,
        }
    }
}

impl Orientation {
    pub fn into_vector3(self) -> Vector3<f32> {
        Vector3::new(self.x, self.y, self.z)
    }
}

pub struct Coordinate {
    pub x: f32, // TODO: u32とかでもいい気がする
    pub y: f32,
    pub z: f32,
}

impl From<Vector3<f32>> for Coordinate {
    fn from(value: Vector3<f32>) -> Self {
        Self {
            x: value[0],
            y: value[1],
            z: value[2],
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    #[test]
    fn coordinate_is_correct() {
        todo!()
    }
}
