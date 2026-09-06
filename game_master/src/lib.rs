use std::{fs::File, future::Future, io::Read, path::Path};

use anyhow::Context;
use derive_more::{From, Into};
use serde::Deserialize;
use time::UtcDateTime;
use tokio::sync::mpsc;

use crate::proto::Difficulty;

pub mod proto {
    include!(concat!(
        env!("OUT_DIR"),
        concat!("/tk75attractions.struckout.v1.rs")
    ));
}

mod data;
pub use data::DataSourceImpl;
mod service;
pub use service::GameMasterServiceImpl;

/// Corresponds to `game_id` column in `games` table.
#[derive(Debug, Clone, Copy, Into, From, PartialEq, Eq, Hash)]
pub struct GameId(u32);

/// Corresponds to `machine_id` column in `games` table.
#[derive(Debug, Clone, Copy, Into, From, PartialEq, Eq, Hash)]
pub struct MachineId(u32);

#[derive(Debug, Clone, Copy, Into, From, PartialEq, Eq, Hash)]
pub struct PlayerId(u32);

pub trait DataSource: Send + Sync + 'static {
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
    ) -> impl Future<Output = Result<PlayerId, sqlx::Error>> + Send;
}

#[derive(Deserialize)]
pub struct Config {
    pub port: u16,
}

impl Config {
    /// Reads configs from specified file.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, anyhow::Error> {
        let mut file = File::open(path).with_context(|| "failed to open config file")?;

        let mut content = String::new();
        file.read_to_string(&mut content)
            .with_context(|| "failed to read config file")?;

        let config: Config =
            toml::from_str(&content).with_context(|| "failed to parse config file")?;
        Ok(config)
    }
}
