//! Conversion between safe domain types and proto's generated types

use struckout_proto::{
    Difficulty,
    types::{GameId, MachineId},
};

/// Events which occur in the service. This can be converted to [`struckout_proto::Event`].
///
/// With this type, you don't need to convert proto types like `u32` or [`prost_types::Duration`]
/// to domain type like [`GameId`] or [`time::SignedDuration`] each time.
#[derive(Debug, Clone)]
pub enum SuccessfulEvent {
    /// [`struckout_proto::event::GameStarted`]
    GameStarted { difficulty: Difficulty },
    /// [`struckout_proto::event::GameTimeLimitNotify`]
    GameTimeLimitNotify { remaining: time::SignedDuration },
    /// [`struckout_proto::event::GameScoreChanged`]
    GameScoreChanged { total_score: u32 },
    /// [`struckout_proto::event::GameFinished`]
    GameFinished { game_id: GameId },
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
            SuccessfulEvent::GameStarted { difficulty } => EventData::GameStarted(GameStarted {
                difficulty: difficulty.into(),
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
