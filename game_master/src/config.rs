use std::{fs, io, path::Path};

use serde::Deserialize;
use thiserror::Error;

use game_master::GameDurations;

const CONFIG_PATH: &str = "/etc/game_master/config.toml";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    game_duration_secs: GameDurations,
}

#[derive(Debug, Error)]
pub enum LoadConfigError {
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
    pub game_duration_secs: GameDurations,
}

impl Config {
    pub fn load() -> Result<Self, LoadConfigError> {
        let file = Self::read_file(Path::new(CONFIG_PATH))?;
        Ok(Self {
            game_duration_secs: file.game_duration_secs,
        })
    }

    fn read_file(path: &Path) -> Result<FileConfig, LoadConfigError> {
        let content = fs::read_to_string(path).map_err(|source| LoadConfigError::Io {
            path: path.display().to_string(),
            source,
        })?;
        toml::from_str(&content).map_err(|source| LoadConfigError::Parse {
            path: path.display().to_string(),
            source,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_CONFIG: &str = "\
[game_duration_secs]
normal = 120
hard = 90
veryhard = 60
";

    #[test]
    fn reads_all_durations() {
        let file: FileConfig = toml::from_str(VALID_CONFIG).unwrap();
        assert_eq!(file.game_duration_secs.normal.get(), 120);
        assert_eq!(file.game_duration_secs.hard.get(), 90);
        assert_eq!(file.game_duration_secs.veryhard.get(), 60);
    }

    #[test]
    fn requires_every_difficulty() {
        assert!(
            toml::from_str::<FileConfig>("[game_duration_secs]\nnormal = 120\nhard = 90").is_err()
        );
        assert!(toml::from_str::<FileConfig>("game_duration_secs = 120").is_err());
    }

    #[test]
    fn rejects_zero_and_unknown_durations() {
        assert!(
            toml::from_str::<FileConfig>(&VALID_CONFIG.replace("hard = 90", "hard = 0")).is_err()
        );
        assert!(toml::from_str::<FileConfig>(&format!("{VALID_CONFIG}extra = 30\n")).is_err());
    }
}
