//! Communication and state shared by the presentation layer.

mod game_master;
pub use game_master::{ConnectError, GameMasterClient, RequestError};
mod remaining_time;
pub use remaining_time::*;
mod session;
pub use session::{Session, SessionPhase};
