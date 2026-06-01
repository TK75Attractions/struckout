//! Data types

use std::fmt::Debug;

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
