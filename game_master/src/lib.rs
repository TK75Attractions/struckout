use std::{fs::File, io::Read, path::Path};

use anyhow::Context;
use serde::Deserialize;

pub mod proto {
    include!(concat!(
        env!("OUT_DIR"),
        concat!("/tk75attractions.struckout.v1.rs")
    ));
}

pub mod service;

/// Corresponds to `game_id` column in `games` table.
#[derive(Debug, Clone, Copy)]
pub struct GameId(i32);

/// Corresponds to `machine_id` column in `games` table.
#[derive(Debug, Clone, Copy)]
pub struct MachineId(i32);

#[derive(Debug, Clone)]
pub struct Game {
    machine_id: MachineId,
    game_id: GameId,
    score: i32,
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
