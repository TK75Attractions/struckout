//! Conversion between safe domain types and proto's generated types

use struckout_proto::{
    Difficulty, Target,
    types::{GameId, MachineId},
};

use crate::{GamePlayerInfo, PlayerInfo};

/// Events which occur in the service. This can be converted to [`struckout_proto::Event`].
///
/// With this type, you don't need to convert proto types like `u32` or [`prost_types::Duration`]
/// to domain type like [`GameId`] or [`time::SignedDuration`] each time.
#[derive(Debug, Clone)]
pub enum SuccessfulEvent {
    /// [`struckout_proto::event::GameStarted`]
    GameStarted {
        difficulty: Difficulty,
        players: GamePlayerInfo,
    },
    /// [`struckout_proto::event::GameTimeLimitNotify`]
    GameTimeLimitNotify { remaining: time::SignedDuration },
    /// [`struckout_proto::event::GameScoreChanged`]
    GameScoreChanged { total_score: u32 },
    /// [`struckout_proto::event::GameFinished`]
    GameFinished { game_id: GameId },
    /// [`struckout_proto::event::TargetAdded`]
    TargetAdded { target: Target },
    /// [`struckout_proto::event::TargetRemoved`]
    TargetRemoved { target_id: String },
    /// [`struckout_proto::event::SessionDifficultyChanged`]
    SessionDifficultyChanged(Difficulty),
    /// [`struckout_proto::event::StartTutorialRequested`]
    StartTutorialRequested(MachineId),
    /// [`struckout_proto::event::NavigateToPreviousTutorialRequested`]
    NavigateToPreviousTutorialRequested(MachineId),
    /// [`struckout_proto::event::NavigateToNextTutorialRequested`]
    NavigateToNextTutorialRequested(MachineId),
    /// [`struckout_proto::event::NavigateToRankingScreenRequested`]
    NavigateToRankingScreenRequested(MachineId),
    /// [`struckout_proto::event::NavigateToHomeScreenRequested`]
    NavigateToHomeScreenRequested(MachineId),
}

impl SuccessfulEvent {
    /// Returns `true` if the successful event is [`SessionDifficultyChanged`].
    ///
    /// [`SessionDifficultyChanged`]: SuccessfulEvent::SessionDifficultyChanged
    #[must_use]
    pub fn is_session_difficulty_changed(&self) -> bool {
        matches!(self, Self::SessionDifficultyChanged(..))
    }
}

impl From<SuccessfulEvent> for struckout_proto::event::EventData {
    fn from(ev: SuccessfulEvent) -> Self {
        use struckout_proto::event::*;
        match ev {
            SuccessfulEvent::GameStarted {
                difficulty,
                players,
            } => EventData::GameStarted(GameStarted {
                game_dur_secs: super::GAME_DURATION
                    .whole_seconds()
                    .try_into()
                    .expect("GAME_DURATION is a positive duration within u32 seconds"),
                difficulty: difficulty.into(),
                player_info: Some(players.into()),
            }),
            SuccessfulEvent::GameScoreChanged { total_score } => {
                EventData::GameScoreChanged(GameScoreChanged { total_score })
            }
            SuccessfulEvent::GameTimeLimitNotify { remaining } => {
                EventData::GameTimeLimitNotify(GameTimeLimitNotify {
                    remaining: Some(prost_types::Duration {
                        seconds: remaining.whole_seconds(),
                        nanos: remaining.subsec_nanoseconds(),
                    }),
                })
            }
            SuccessfulEvent::GameFinished { game_id } => EventData::GameFinished(GameFinished {
                game_id: game_id.into(),
            }),
            SuccessfulEvent::TargetAdded { target } => EventData::TargetAdded(TargetAdded {
                target: Some(target),
            }),
            SuccessfulEvent::TargetRemoved { target_id } => {
                EventData::TargetRemoved(TargetRemoved { target_id })
            }
            SuccessfulEvent::SessionDifficultyChanged(difficulty) => {
                EventData::SessionDifficultyChanged(SessionDifficultyChanged {
                    difficulty: difficulty.into(),
                })
            }
            SuccessfulEvent::StartTutorialRequested(machine_id) => {
                EventData::StartTutorialRequested(StartTutorialRequested {
                    machine_id: machine_id.into(),
                })
            }
            SuccessfulEvent::NavigateToPreviousTutorialRequested(machine_id) => {
                EventData::NavigateToPreviousTutorialRequested(
                    NavigateToPreviousTutorialRequested {
                        machine_id: machine_id.into(),
                    },
                )
            }
            SuccessfulEvent::NavigateToNextTutorialRequested(machine_id) => {
                EventData::NavigateToNextTutorialRequested(NavigateToNextTutorialRequested {
                    machine_id: machine_id.into(),
                })
            }
            SuccessfulEvent::NavigateToRankingScreenRequested(machine_id) => {
                EventData::NavigateToRankingScreenRequested(NavigateToRankingScreenRequested {
                    machine_id: machine_id.into(),
                })
            }
            SuccessfulEvent::NavigateToHomeScreenRequested(machine_id) => {
                EventData::NavigateToHomeScreenRequsted(NavigateToHomeScreenRequested {
                    machine_id: machine_id.into(),
                })
            }
        }
    }
}

impl From<PlayerInfo> for struckout_proto::player_info::PlayerItem {
    fn from(player: PlayerInfo) -> Self {
        Self {
            machine_id: player.machine_id.into(),
            player_id: player.player_id.into(),
            player_name: player.player_name,
        }
    }
}

impl From<GamePlayerInfo> for struckout_proto::PlayerInfo {
    fn from(players: GamePlayerInfo) -> Self {
        use struckout_proto::player_info::{Multi, PlayerInfoData, Single};
        Self {
            player_info_data: Some(match players {
                GamePlayerInfo::Single(player) => PlayerInfoData::Single(Single {
                    player: Some(player.into()),
                }),
                GamePlayerInfo::Multi(first, second) => PlayerInfoData::Multi(Multi {
                    player_1: Some(first.into()),
                    player_2: Some(second.into()),
                }),
            }),
        }
    }
}
