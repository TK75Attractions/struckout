use std::{fs, io, num::NonZeroU32, path::Path};

use serde::Deserialize;
use thiserror::Error;

use game_master::{ComboBonuses, GameDurations};

const CONFIG_PATH: &str = "/etc/game_master/config.toml";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    game_start_delay_secs: NonZeroU32,
    game_duration_secs: GameDurations,
    combo_bonuses: ComboBonuses,
    pocket_watch_bonus_secs: NonZeroU32,
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
    pub game_start_delay_secs: NonZeroU32,
    pub game_duration_secs: GameDurations,
    pub combo_bonuses: ComboBonuses,
    pub pocket_watch_bonus_secs: NonZeroU32,
}

impl Config {
    pub fn load() -> Result<Self, LoadConfigError> {
        let file = Self::read_file(Path::new(CONFIG_PATH))?;
        Ok(Self {
            game_start_delay_secs: file.game_start_delay_secs,
            game_duration_secs: file.game_duration_secs,
            combo_bonuses: file.combo_bonuses,
            pocket_watch_bonus_secs: file.pocket_watch_bonus_secs,
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
game_start_delay_secs = 3
pocket_watch_bonus_secs = 5
[game_duration_secs]
normal = 120
hard = 90
veryhard = 60

[[combo_bonuses]]
combo = 3
points = 500

[[combo_bonuses]]
combo = 5
points = 800
";

    #[test]
    fn default_config_file_is_valid() {
        toml::from_str::<FileConfig>(include_str!("../../default_configs/game_master.toml"))
            .expect("default_configs/game_master.toml should be a complete game_master config");
    }

    #[test]
    fn reads_all_durations() {
        let file: FileConfig = toml::from_str(VALID_CONFIG).unwrap();
        assert_eq!(file.game_duration_secs.normal.get(), 120);
        assert_eq!(file.game_duration_secs.hard.get(), 90);
        assert_eq!(file.game_duration_secs.veryhard.get(), 60);
    }

    #[test]
    fn requires_positive_game_start_delay() {
        assert!(
            toml::from_str::<FileConfig>(&VALID_CONFIG.replace("game_start_delay_secs = 3\n", ""))
                .is_err()
        );
        assert!(
            toml::from_str::<FileConfig>(
                &VALID_CONFIG.replace("game_start_delay_secs = 3", "game_start_delay_secs = 0")
            )
            .is_err()
        );
    }

    #[test]
    fn requires_every_difficulty() {
        assert!(
            toml::from_str::<FileConfig>(&VALID_CONFIG.replace("veryhard = 60\n", "")).is_err()
        );
        assert!(
            toml::from_str::<FileConfig>(
                "game_start_delay_secs = 3\npocket_watch_bonus_secs = 5\ncombo_bonuses = []\ngame_duration_secs = 120\n"
            )
            .is_err()
        );
    }

    #[test]
    fn rejects_zero_and_unknown_durations() {
        assert!(
            toml::from_str::<FileConfig>(&VALID_CONFIG.replace("hard = 90", "hard = 0")).is_err()
        );
        assert!(toml::from_str::<FileConfig>(&format!("{VALID_CONFIG}extra = 30\n")).is_err());
    }

    #[test]
    fn requires_combo_bonuses() {
        assert!(
            toml::from_str::<FileConfig>(
                "game_start_delay_secs = 3\npocket_watch_bonus_secs = 5\n[game_duration_secs]\nnormal = 120\nhard = 90\nveryhard = 60\n"
            )
            .is_err()
        );
    }

    #[test]
    fn requires_positive_pocket_watch_bonus() {
        assert!(
            toml::from_str::<FileConfig>(
                &VALID_CONFIG.replace("pocket_watch_bonus_secs = 5\n", "")
            )
            .is_err()
        );
        assert!(
            toml::from_str::<FileConfig>(
                &VALID_CONFIG.replace("pocket_watch_bonus_secs = 5", "pocket_watch_bonus_secs = 0")
            )
            .is_err()
        );
    }

    #[test]
    fn rejects_invalid_combo_bonuses() {
        assert!(
            toml::from_str::<FileConfig>(&VALID_CONFIG.replace("combo = 5", "combo = 3")).is_err()
        );
        assert!(
            toml::from_str::<FileConfig>(&VALID_CONFIG.replace("combo = 3", "combo = 0")).is_err()
        );
        assert!(
            toml::from_str::<FileConfig>(&VALID_CONFIG.replace("points = 500", "points = 0"))
                .is_err()
        );
        assert!(
            toml::from_str::<FileConfig>(&VALID_CONFIG.replace("points = 500", "points = -1"))
                .is_err()
        );
        assert!(
            toml::from_str::<FileConfig>(
                &VALID_CONFIG.replace("points = 500", "points = 500\nextra = 1")
            )
            .is_err()
        );
    }

    #[test]
    fn permits_explicitly_empty_combo_bonuses() {
        assert!(
            toml::from_str::<FileConfig>(
                "game_start_delay_secs = 3\npocket_watch_bonus_secs = 5\ncombo_bonuses = []\n[game_duration_secs]\nnormal = 120\nhard = 90\nveryhard = 60\n"
            )
            .is_ok()
        );
    }
}
