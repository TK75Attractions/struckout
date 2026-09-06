use std::{fs::File, io::Read, path::Path};

use anyhow::Context;
use derive_more::{From, Into};
use serde::Deserialize;

pub mod proto {
    include!(concat!(
        env!("OUT_DIR"),
        concat!("/tk75attractions.struckout.v1.rs")
    ));
}

mod data;
pub mod service;

/// Corresponds to `game_id` column in `games` table.
#[derive(Debug, Clone, Copy, Into, From, PartialEq, Eq, Hash)]
pub struct GameId(u32);

/// Corresponds to `machine_id` column in `games` table.
#[derive(Debug, Clone, Copy, Into, From, PartialEq, Eq, Hash)]
pub struct MachineId(u32);

#[derive(Debug, Clone, Copy, Into, From, PartialEq, Eq, Hash)]
pub struct PlayerId(u32);

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
