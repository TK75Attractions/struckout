mod ui {
    slint::include_modules!();

    #[stern::route]
    #[derive(Debug, Clone)]
    pub enum NavRoute {
        Start,
        NameInput,
        DifficulitySelect {
            player_id: struckout_proto::types::PlayerId,
        },
        Playing(self::Difficulity),
        Score,
        Ranking,
        Fallback(String),
        ConnectionFailed(String),
        Connecting,
    }
}

pub use ui::*;

impl From<crate::Difficulity> for struckout_proto::Difficulty {
    fn from(value: crate::Difficulity) -> Self {
        match value {
            crate::Difficulity::Normal => struckout_proto::Difficulty::Normal,
            crate::Difficulity::Hard => struckout_proto::Difficulty::Hard,
            crate::Difficulity::VeryHard => struckout_proto::Difficulty::Veryhard,
        }
    }
}
