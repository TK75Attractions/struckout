use std::future::Future;

use thiserror::Error;
use time::UtcDateTime;

use struckout_proto::{
    Difficulty,
    types::{GameId, MachineId, PlayerId},
};

mod data;
pub use data::DataSourceImpl;
mod service;
pub use service::GameMasterServiceImpl;

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
