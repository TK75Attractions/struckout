//! Similar to Data Layer in Android.
//! This module deals with data sources (e.g. database).

mod game_master;
pub use game_master::{ConnectError, RequestError, Session};
mod remaining_time;
pub use remaining_time::*;
use struckout_proto::game_master_service_client::GameMasterServiceClient;

pub type GameMasterClient =
    game_master::GameMasterClient<GameMasterServiceClient<tonic::transport::Channel>>;
//pub mod player;
//pub mod projector;
