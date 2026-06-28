mod sync;
pub use sync::SyncArgs;

mod proto {
    include!(concat!(env!("OUT_DIR"), "/tk75attractions.struckout.v1.rs"));
}
