use struckout_proto::{
    Difficulty, Event,
    event::{EventData, game_started::GameStartedPlayerInfo},
    types::{GameId, MachineId, PlayerId},
};

use super::{DisplayableRemainingTime, RequestError};

/// A snapshot keeps completion available even when the UI subscribes after the event.
#[derive(Debug, Clone, Default)]
pub struct Session {
    pub phase: SessionPhase,
    pub difficulty: Option<Difficulty>,
    pub score: u32,
    pub remaining_time: DisplayableRemainingTime,
    pub error: Option<RequestError>,
    pub(super) player_id: Option<PlayerId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SessionPhase {
    #[default]
    Idle,
    ConfiguringMode,
    ConfiguringDifficulty,
    Ready,
    Running,
    Finished(GameId),
}

impl Session {
    pub(super) fn apply_event(
        &mut self,
        machine_id: MachineId,
        event: Event,
    ) -> Result<(), RequestError> {
        let data = event
            .event_data
            .ok_or_else(|| RequestError::missing_field("Event", "event_data"))?;
        match data {
            EventData::GameStarted(started)
                if matches!(
                    self.phase,
                    SessionPhase::ConfiguringDifficulty | SessionPhase::Ready
                ) =>
            {
                let players = started.game_started_player_info.ok_or_else(|| {
                    RequestError::missing_field("GameStarted", "game_started_player_info")
                })?;
                let players = match players {
                    GameStartedPlayerInfo::Single(single) => {
                        vec![single.player.ok_or_else(|| {
                            RequestError::missing_field("GameStarted.Single", "player")
                        })?]
                    }
                    GameStartedPlayerInfo::Multi(multi) => vec![
                        multi.player_1.ok_or_else(|| {
                            RequestError::missing_field("GameStarted.Multi", "player_1")
                        })?,
                        multi.player_2.ok_or_else(|| {
                            RequestError::missing_field("GameStarted.Multi", "player_2")
                        })?,
                    ],
                };
                // All-machine events can also describe another machine's game.
                if !players.iter().any(|player| {
                    player.machine_id == machine_id.into_inner()
                        && Some(PlayerId::new(player.player_id)) == self.player_id
                }) {
                    return Ok(());
                }
                let difficulty = parse_difficulty(started.difficulty)?;
                self.difficulty = Some(difficulty);
                self.phase = SessionPhase::Running;
                self.score = 0;
                self.remaining_time = DisplayableRemainingTime::ZERO;
            }
            EventData::SessionDifficultyChanged(changed)
                if matches!(
                    self.phase,
                    SessionPhase::ConfiguringMode
                        | SessionPhase::ConfiguringDifficulty
                        | SessionPhase::Ready
                ) =>
            {
                self.difficulty = Some(parse_difficulty(changed.difficulty)?);
            }
            EventData::GameTimeLimitNotify(notify) if self.phase == SessionPhase::Running => {
                self.remaining_time = notify
                    .remaining
                    .ok_or_else(|| RequestError::missing_field("GameTimeLimitNotify", "remaining"))?
                    .try_into()
                    .map_err(|_| {
                        RequestError::invalid_packet(
                            "GameTimeLimitNotify",
                            "remaining",
                            "the duration was invalid",
                        )
                    })?;
            }
            EventData::GameScoreChanged(changed) if self.phase == SessionPhase::Running => {
                self.score = changed.total_score;
            }
            EventData::GameFinished(finished) if self.phase == SessionPhase::Running => {
                self.phase = SessionPhase::Finished(GameId::new(finished.game_id));
            }
            // Targets and projector navigation do not change the touchpanel's game state.
            _ => {}
        }
        Ok(())
    }
}

fn parse_difficulty(value: i32) -> Result<Difficulty, RequestError> {
    match Difficulty::try_from(value) {
        Ok(Difficulty::Unspecified) | Err(_) => Err(RequestError::invalid_packet(
            "Event",
            "difficulty",
            "unknown or unspecified difficulty",
        )),
        Ok(difficulty) => Ok(difficulty),
    }
}

#[cfg(test)]
mod tests {
    use struckout_proto::event::{
        GameFinished, GameScoreChanged, GameStarted, GameTimeLimitNotify, SessionDifficultyChanged,
        game_started::{GameStartedPlayerInfo, Multi, PlayerInfo},
    };

    use super::*;

    fn event(data: EventData) -> Event {
        Event {
            event_data: Some(data),
        }
    }

    #[test]
    fn multi_game_uses_the_player_on_this_machine() {
        let mut session = Session {
            phase: SessionPhase::Ready,
            player_id: Some(PlayerId::new(12)),
            ..Session::default()
        };
        let started = EventData::GameStarted(GameStarted {
            difficulty: Difficulty::Hard.into(),
            game_started_player_info: Some(GameStartedPlayerInfo::Multi(Multi {
                player_1: Some(PlayerInfo {
                    machine_id: 2,
                    player_id: 34,
                }),
                player_2: Some(PlayerInfo {
                    machine_id: 1,
                    player_id: 12,
                }),
            })),
        });
        session
            .apply_event(MachineId::new(1), event(started))
            .expect("valid event");
        assert_eq!(session.phase, SessionPhase::Running);
        assert_eq!(session.difficulty, Some(Difficulty::Hard));
    }

    #[test]
    fn unrelated_game_and_idle_events_do_not_change_the_session() {
        let mut session = Session::default();
        session
            .apply_event(
                MachineId::new(1),
                event(EventData::GameFinished(GameFinished { game_id: 42 })),
            )
            .expect("ignored event is valid");
        session
            .apply_event(
                MachineId::new(1),
                event(EventData::SessionDifficultyChanged(
                    SessionDifficultyChanged {
                        difficulty: Difficulty::Hard.into(),
                    },
                )),
            )
            .expect("ignored event is valid");
        assert_eq!(session.phase, SessionPhase::Idle);
        assert_eq!(session.difficulty, None);
    }

    #[test]
    fn running_snapshot_tracks_server_score_time_and_completion() {
        let mut session = Session {
            phase: SessionPhase::Running,
            ..Session::default()
        };
        for data in [
            EventData::GameScoreChanged(GameScoreChanged { total_score: 123 }),
            EventData::GameTimeLimitNotify(GameTimeLimitNotify {
                remaining: Some(prost_types::Duration {
                    seconds: 95,
                    nanos: 0,
                }),
            }),
            EventData::GameFinished(GameFinished { game_id: 42 }),
        ] {
            session
                .apply_event(MachineId::new(1), event(data))
                .expect("valid event");
        }
        assert_eq!(session.score, 123);
        assert_eq!(session.remaining_time.to_string(), "01:35");
        assert_eq!(session.phase, SessionPhase::Finished(GameId::new(42)));
    }

    #[test]
    fn negative_remaining_time_is_rejected() {
        let mut session = Session {
            phase: SessionPhase::Running,
            ..Session::default()
        };
        assert!(
            session
                .apply_event(
                    MachineId::new(1),
                    event(EventData::GameTimeLimitNotify(GameTimeLimitNotify {
                        remaining: Some(prost_types::Duration {
                            seconds: -1,
                            nanos: 0,
                        }),
                    })),
                )
                .is_err()
        );
    }
}
