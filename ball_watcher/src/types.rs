//! Data types and conversion utilities

use std::fmt::Debug;

use nalgebra::Vector3;

use crate::{
    protobuf::{CameraLocation, DetectedObject},
    triangulate::Position3D,
};

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

pub trait ToVector3 {
    fn to_vector3(&self) -> Vector3<f64>;
}

impl ToVector3 for CameraLocation {
    fn to_vector3(&self) -> Vector3<f64> {
        Vector3::new(self.x, self.y, self.z)
    }
}

impl ToVector3 for Position3D {
    fn to_vector3(&self) -> Vector3<f64> {
        Vector3::new(self.x, self.y, self.z)
    }
}

pub trait GetLayFromDetectedObject {
    fn get_lay(&self) -> Vector3<f64>;
}

impl GetLayFromDetectedObject for DetectedObject {
    fn get_lay(&self) -> Vector3<f64> {
        Vector3::new(self.lay_x, self.lay_y, self.lay_z)
    }
}
