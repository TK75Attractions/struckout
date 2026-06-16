use nalgebra::Vector3;

pub(crate) fn triangulate(
    camera_loc_1: Vector3<f64>,
    orientation_1: Vector3<f64>,
    camera_loc_2: Vector3<f64>,
    orientation_2: Vector3<f64>,
) -> Position3D {
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

pub struct Position3D {
    pub x: f64, // TODO: u32とかでもいい気がする
    pub y: f64,
    pub z: f64,
}

impl From<Vector3<f64>> for Position3D {
    fn from(value: Vector3<f64>) -> Self {
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
