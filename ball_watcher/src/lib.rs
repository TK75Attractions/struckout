pub(crate) mod transport;
pub(crate) mod triangulate;
pub(crate) mod types;

pub mod protobuf {
    include!(concat!(env!("OUT_DIR"), "/struckout.rs"));
}

pub fn run_main() {}
