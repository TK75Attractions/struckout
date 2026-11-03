//! Similar to Data Layer in Android.
//! This module deals with data sources (e.g. database).

mod game_master;
pub use game_master::{ConnectError, RequestError, Session};
mod remaining_time;
pub use remaining_time::*;
pub type GameMasterClient = game_master::GameMasterClient<game_master::GameMasterGrpcClient>;
