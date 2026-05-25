//! Data types

use bt_hci::param::ConnHandle;
use core::fmt::Debug;
use nalgebra::Vector3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FrameId(u32);

#[cfg(feature = "defmt")]
impl defmt::Format for FrameId {
    fn format(&self, fmt: defmt::Formatter) {
        self.0.format(fmt);
    }
}

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

/// フレームのデータ。
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Frame {
    pub conn_id: ConnHandle,
    pub frame_id: FrameId,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Frame {
    pub fn from_bytes(data: [u8; 16], conn_id: ConnHandle) -> Self {
        let frame_id = data[..4].try_into().unwrap();
        let frame_id = FrameId(u32::from_le_bytes(frame_id));
        let x = data[4..8].try_into().unwrap();
        let x = f32::from_le_bytes(x);
        let y = data[8..12].try_into().unwrap();
        let y = f32::from_le_bytes(y);
        let z = data[12..].try_into().unwrap();
        let z = f32::from_le_bytes(z);
        Self {
            conn_id,
            frame_id,
            x,
            y,
            z,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CameraLocation {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl CameraLocation {
    pub fn from_bytes(data: [u8; 12]) -> Self {
        let x = data[0..4].try_into().unwrap();
        let x = f32::from_le_bytes(x);
        let y = data[4..8].try_into().unwrap();
        let y = f32::from_le_bytes(y);
        let z = data[8..].try_into().unwrap();
        let z = f32::from_le_bytes(z);
        Self { x, y, z }
    }
}

impl CameraLocation {
    pub fn into_vector3(self) -> Vector3<f32> {
        Vector3::new(self.x, self.y, self.z)
    }
}
