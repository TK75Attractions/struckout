use std::future::Future;

use thiserror::Error;
use time::UtcDateTime;

use struckout_proto::{
    Difficulty,
    types::{MachineId, PlayerId},
};

mod data;
pub use data::DataSourceImpl;
pub mod health;
mod services;
pub use services::{GameMasterPlayerServiceImpl, GameMasterSessionServiceImpl};
mod session;

use crate::data::Player;

pub trait DataSource: Clone + Send + Sync + 'static {
    fn save_game(&self, game: GameRecord)
    -> impl Future<Output = Result<(), SaveGameError>> + Send;

    fn add_player(
        &self,
        name: impl Into<String> + Send,
    ) -> impl Future<Output = Result<PlayerId, AddPlayerError>> + Send;

    fn validate_player_name(
        &self,
        name: impl Into<String> + Send,
    ) -> impl Future<Output = Result<(), ValidatePlayerNameError>> + Send;

    fn get_player(
        &self,
        name: impl Into<String> + Send,
    ) -> impl Future<Output = Result<Player, GetPlayerError>> + Send;
}

/// Error returned from [`DataSource::add_player()`].
#[derive(Debug, Error)]
pub enum AddPlayerError {
    #[error("player name is already used")]
    NameAlreadyUsed,
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
}

/// A completed game's participants, preventing invalid single/multi combinations.
#[derive(Debug, Clone, PartialEq)]
pub enum GamePlayers {
    Single {
        machine_id: MachineId,
        player_id: PlayerId,
    },
    Multi {
        player_ids: [PlayerId; 2],
    },
}

/// Retains the final result until session completion makes it ready for persistence.
#[derive(Debug, Clone, PartialEq)]
pub struct GameRecord {
    pub players: GamePlayers,
    pub started_at: UtcDateTime,
    pub difficulty: Difficulty,
    pub score: u32,
}

/// Error returned from [`DataSource::save_game()`].
#[derive(Debug, Error)]
pub enum SaveGameError {
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

/// Error returned from [`DataSource::get_player()`].
#[derive(Debug, Error)]
pub enum GetPlayerError {
    #[error("Player with name '{0}' does not exist")]
    NotExist(String),
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
}
