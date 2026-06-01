use nalgebra::Vector3;

pub(crate) fn calc_coordinate(
    camera_loc_1: Vector3<f32>,
    orientation_1: Orientation,
    camera_loc_2: Vector3<f32>,
    orientation_2: Orientation,
) -> Coordinate {
    // TODO: From / Intoを使う
    let p = camera_loc_1;
    let q = camera_loc_2;
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
