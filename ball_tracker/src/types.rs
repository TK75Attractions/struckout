//! Data types and conversion utilities

use std::fmt::Debug;

use nalgebra::{Rotation3, Vector3};
use serde::{Deserialize, Serialize};
use struckout_proto::{CameraPose, Detection};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FrameId(u32);

impl FrameId {
    // panics when conversion failed.
    pub fn new<T>(val: T) -> Self
    where
        T: TryInto<u32>,
        <T as TryInto<u32>>::Error: Debug,
    {
        Self(val.try_into().unwrap())
    }

    pub fn try_new(val: impl TryInto<u32>) -> Option<Self> {
        val.try_into().map(Self).ok()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CameraId(u32);

impl CameraId {
    pub fn new(val: u32) -> Self {
        Self(val)
    }
}

impl From<u32> for CameraId {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone, Copy)]
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CollisionPoint3D {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

pub trait ToVector3 {
    fn to_vector3(&self) -> Vector3<f64>;
}

impl ToVector3 for CameraPose {
    fn to_vector3(&self) -> Vector3<f64> {
        Vector3::new(self.x, self.y, self.z)
    }
}

pub trait RotateToWorld {
    fn rotate_to_world(&self, direction: Vector3<f64>) -> Vector3<f64>;
}

impl RotateToWorld for CameraPose {
    fn rotate_to_world(&self, direction: Vector3<f64>) -> Vector3<f64> {
        Rotation3::from_euler_angles(
            self.rotation_x_degrees.to_radians(),
            self.rotation_y_degrees.to_radians(),
            self.rotation_z_degrees.to_radians(),
        ) * direction
    }
}

impl ToVector3 for Position3D {
    fn to_vector3(&self) -> Vector3<f64> {
        Vector3::new(self.x, self.y, self.z)
    }
}

pub trait GetLayFromDetection {
    fn get_lay(&self) -> Vector3<f64>;
}

impl GetLayFromDetection for Detection {
    fn get_lay(&self) -> Vector3<f64> {
        Vector3::new(self.lay_x, self.lay_y, self.lay_z)
    }
}

#[cfg(test)]
mod tests {
    use approx::assert_relative_eq;

    use super::*;

    fn camera(rotation_x: f64, rotation_y: f64, rotation_z: f64) -> CameraPose {
        CameraPose {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            rotation_x_degrees: rotation_x,
            rotation_y_degrees: rotation_y,
            rotation_z_degrees: rotation_z,
        }
    }

    #[test]
    fn identity_pose_keeps_device_direction() {
        let direction = Vector3::new(1.0, 2.0, 3.0);

        let world_direction = camera(0.0, 0.0, 0.0).rotate_to_world(direction);

        assert_relative_eq!(world_direction, direction);
    }

    #[test]
    fn rotates_device_direction_into_world_coordinates() {
        let direction = Vector3::new(1.0, 0.0, 0.0);

        let world_direction = camera(0.0, 0.0, 90.0).rotate_to_world(direction);

        assert_relative_eq!(world_direction.x, 0.0, epsilon = 1e-12);
        assert_relative_eq!(world_direction.y, 1.0, epsilon = 1e-12);
        assert_relative_eq!(world_direction.z, 0.0, epsilon = 1e-12);
    }
}
