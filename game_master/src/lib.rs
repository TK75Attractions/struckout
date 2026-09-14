use std::future::Future;

use thiserror::Error;
use time::UtcDateTime;

use struckout_proto::Difficulty;

mod data;
pub use data::DataSourceImpl;
mod service;
pub use service::GameMasterServiceImpl;

/// Defines a new-type for id.
macro_rules! id_new_type {
    ($new_type:ident($inner_type:ty)) => {
        #[derive(Debug, Clone, Copy, derive_more::Into, derive_more::From, PartialEq, Eq, Hash)]
        pub struct $new_type($inner_type);

        impl $new_type {
            /// Returns inner value of self.
            pub fn into_inner(self) -> $inner_type {
                <$new_type as Into<$inner_type>>::into(self)
            }
        }
    };
}

id_new_type!(GameId(u32));

id_new_type!(MachineId(u32));

id_new_type!(PlayerId(u32));

pub trait DataSource: Clone + Send + Sync + 'static {
    fn insert_game(
        &self,
        machine_id: MachineId,
        player_id: PlayerId,
        started_at: UtcDateTime,
        difficulty: Difficulty,
    ) -> impl Future<Output = Result<GameId, sqlx::Error>> + Send;

    fn complete_game(
        &self,
        game_id: GameId,
        score: u32,
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send;

    fn add_player(
        &self,
        name: impl Into<String> + Send,
    ) -> impl Future<Output = Result<PlayerId, AddPlayerError>> + Send;
}

/// Error returned from [`DataSource::add_player()`].
#[derive(Debug, Error)]
pub enum AddPlayerError {
    #[error("player name is already used")]
    NameAlreadyUsed,
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
}
