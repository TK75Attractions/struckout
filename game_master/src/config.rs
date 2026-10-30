use std::{fs, io, num::NonZeroU32, path::Path};

use serde::Deserialize;
use thiserror::Error;

use crate::Cli;

const CONFIG_PATH: &str = "/etc/game_master/config.toml";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    game_duration_secs: Option<NonZeroU32>,
}

#[derive(Debug, Error)]
pub enum LoadConfigError {
    #[error("--game-duration-secs か設定ファイルの game_duration_secs を指定してください")]
    MissingGameDuration,
    #[error("設定ファイル {path} を読めません: {source}")]
    Io {
        path: String,
        #[source]
        source: io::Error,
    },
    #[error("設定ファイル {path} の形式が正しくありません: {source}")]
    Parse {
        path: String,
        #[source]
        source: toml::de::Error,
    },
}

pub struct Config {
    pub game_duration_secs: NonZeroU32,
}

impl Config {
    pub fn load(cli: Cli) -> Result<Self, LoadConfigError> {
        let file = Self::read_file(Path::new(CONFIG_PATH))?;
        Self::from_file_and_cli(file, cli)
    }

    fn from_file_and_cli(file: Option<FileConfig>, cli: Cli) -> Result<Self, LoadConfigError> {
        let game_duration_secs = cli
            .game_duration_secs
            .or(file.and_then(|config| config.game_duration_secs))
            .ok_or(LoadConfigError::MissingGameDuration)?;
        Ok(Self { game_duration_secs })
    }

    fn read_file(path: &Path) -> Result<Option<FileConfig>, LoadConfigError> {
        let content = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(source) => {
                return Err(LoadConfigError::Io {
                    path: path.display().to_string(),
                    source,
                });
            }
        };
        toml::from_str(&content)
            .map(Some)
            .map_err(|source| LoadConfigError::Parse {
                path: path.display().to_string(),
                source,
            })
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use clap::Parser;

    use super::*;

    #[test]
    fn cli_overrides_file() {
        let file: FileConfig = toml::from_str("game_duration_secs = 120").unwrap();
        let cli = Cli {
            game_duration_secs: Some(NonZeroU32::new(30).unwrap()),
        };
        assert_eq!(
            Config::from_file_and_cli(Some(file), cli)
                .unwrap()
                .game_duration_secs
                .get(),
            30
        );
    }

    #[test]
    fn uses_file_and_requires_value() {
        let cli = Cli {
            game_duration_secs: None,
        };
        let file: FileConfig = toml::from_str("game_duration_secs = 120").unwrap();
        assert_eq!(
            Config::from_file_and_cli(Some(file), cli)
                .unwrap()
                .game_duration_secs
                .get(),
            120
        );
        assert!(matches!(
            Config::from_file_and_cli(
                None,
                Cli {
                    game_duration_secs: None
                }
            ),
            Err(LoadConfigError::MissingGameDuration)
        ));
    }

    #[test]
    fn rejects_zero_duration() {
        assert!(toml::from_str::<FileConfig>("game_duration_secs = 0").is_err());
        assert!(Cli::try_parse_from(["game_master", "--game-duration-secs", "0"]).is_err());
    }

    #[test]
    fn reads_optional_config_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        assert!(Config::read_file(&path).unwrap().is_none());

        fs::write(&path, "game_duration_secs = 120").unwrap();
        assert_eq!(
            Config::read_file(&path)
                .unwrap()
                .unwrap()
                .game_duration_secs
                .unwrap()
                .get(),
            120
        );

        fs::write(&path, "game_duration_secs = 0").unwrap();
        assert!(matches!(
            Config::read_file(&path),
            Err(LoadConfigError::Parse { .. })
        ));
    }
}
