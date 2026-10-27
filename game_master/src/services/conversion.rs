//! Conversion between safe domain types and proto's generated types

use struckout_proto::{
    Difficulty, Target,
    types::{GameId, MachineId},
};

use crate::GamePlayers;

/// Events which occur in the service. This can be converted to [`struckout_proto::Event`].
///
/// With this type, you don't need to convert proto types like `u32` or [`prost_types::Duration`]
/// to domain type like [`GameId`] or [`time::SignedDuration`] each time.
#[derive(Debug, Clone)]
pub enum SuccessfulEvent {
    /// [`struckout_proto::event::GameStarted`]
    GameStarted {
        difficulty: Difficulty,
        players: GamePlayers,
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
                game_started_player_info: Some(match players {
                    GamePlayers::Single {
                        machine_id,
                        player_id,
                    } => game_started::GameStartedPlayerInfo::Single(game_started::Single {
                        player: Some(game_started::PlayerInfo {
                            machine_id: machine_id.into(),
                            player_id: player_id.into(),
                        }),
                    }),
                    GamePlayers::Multi(
                        (machine_id_1, player_id_1),
                        (machine_id_2, player_id_2),
                    ) => game_started::GameStartedPlayerInfo::Multi(game_started::Multi {
                        player_1: Some(game_started::PlayerInfo {
                            machine_id: machine_id_1.into(),
                            player_id: player_id_1.into(),
                        }),
                        player_2: Some(game_started::PlayerInfo {
                            machine_id: machine_id_2.into(),
                            player_id: player_id_2.into(),
                        }),
                    }),
                }),
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
        }
    }
}
