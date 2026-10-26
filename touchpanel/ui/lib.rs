mod ui {
    slint::include_modules!();

    #[stern::route]
    #[derive(Debug, Clone)]
    pub enum NavRoute {
        Start,
        NameInputFirst,
        ModeSelect {
            player_id: struckout_proto::types::PlayerId,
        },
        DifficulitySelect,
        Playing,
        GameFinished,
        RankingControl,
        Fallback(String),
        ConnectionFailed(String),
        Connecting,
        NameInputNonFirst, //@xtask-route
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

impl From<struckout_proto::Difficulty> for crate::Difficulity {
    fn from(value: struckout_proto::Difficulty) -> Self {
        match value {
            struckout_proto::Difficulty::Normal | struckout_proto::Difficulty::Unspecified => {
                Self::Normal
            }
            struckout_proto::Difficulty::Hard => Self::Hard,
            struckout_proto::Difficulty::Veryhard => Self::VeryHard,
        }
    }
}
