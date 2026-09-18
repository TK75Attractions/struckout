mod ui {
    slint::include_modules!();

    #[stern::route]
    #[derive(Debug, Clone)]
    pub enum NavRoute {
        Start,
        NameInput,
        DifficulitySelect { player_id: crate::types::PlayerId },
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

pub mod types {
    /// Defines a new-type for id.
    macro_rules! id_new_type {
        ($new_type:ident($inner_type:ty)) => {
            #[derive(
                Debug, Clone, Copy, derive_more::Into, derive_more::From, PartialEq, Eq, Hash,
            )]
            pub struct $new_type($inner_type);

            impl $new_type {
                pub fn new(value: $inner_type) -> Self {
                    Self(value)
                }

                /// Returns inner value of self.
                pub fn into_inner(self) -> $inner_type {
                    <$new_type as Into<$inner_type>>::into(self)
                }
            }
        };
    }

    id_new_type!(PlayerId(u32));

    id_new_type!(GameId(u32));

    id_new_type!(MachineId(u32));
}
