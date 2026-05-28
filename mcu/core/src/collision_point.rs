use heapless::Vec;
use nalgebra::{Vector, Vector6};

use crate::calc_coordinate::Coordinate;

const COORDINATE_HISTORY_CAP: usize = 50;

pub(crate) struct CollisionPointCalculator {
    coordinate_history: Vec<Coordinate, COORDINATE_HISTORY_CAP>,
}

impl CollisionPointCalculator {
    fn calc_collision_point(&self) -> Coordinate {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
}
