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

use crate::data::GameRecord;

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

    fn get_game_result(
        &self,
        game_id: GameId,
    ) -> impl Future<Output = Result<GameRecord, GetGameResultError>> + Send;

    fn validate_player_name(
        &self,
        name: impl Into<String> + Send,
    ) -> impl Future<Output = Result<(), ValidatePlayerNameError>> + Send;
}

/// Error returned from [`DataSource::add_player()`].
#[derive(Debug, Error)]
pub enum AddPlayerError {
    #[error("player name is already used")]
    NameAlreadyUsed,
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
}

/// Error returned from [`DataSource::get_game_result()`].
#[derive(Debug, Error)]
pub enum GetGameResultError {
    #[error("game is not yet completed")]
    NotYetCompleted,
    #[error("game not found in the database")]
    GameNotFound,
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
}

/// Error returned from [`DataSource::validate_player_name()`].
#[derive(Debug, Error)]
pub enum ValidatePlayerNameError {
    #[error("Player name {0} is already used")]
    AlreadyUsed(String),
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
}
