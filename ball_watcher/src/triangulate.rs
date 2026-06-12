use nalgebra::Vector3;

pub(crate) fn triangulate(
    camera_loc_1: Vector3<f32>,
    orientation_1: Vector3<f32>,
    camera_loc_2: Vector3<f32>,
    orientation_2: Vector3<f32>,
) -> Coordinate {
    // TODO: From / Intoを使う
    let p = camera_loc_1;
    let q = camera_loc_2;
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
