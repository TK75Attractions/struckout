//! Session management

use std::{collections::HashMap, sync::Arc};

use parking_lot::RwLock;

use struckout_macros::{EnumAsInner, EnumAsInnerMut};
use struckout_proto::{
    Difficulty, GameMode, Target,
    types::{MachineId, PlayerId},
};
use thiserror::Error;
use tracing::trace;
use uuid::Uuid;

use crate::services::EventTarget;

/// Owns machine sessions so configuration and readiness share a single source of state.
///
/// Sessions begin in [`MachineSession::ConfiguringGameMode`]. Selecting a mode with
/// [`Self::set_game_mode`] advances to difficulty configuration; [`Self::update_difficulty`]
/// and [`Self::set_ready`] then configure difficulty and readiness respectively.
#[derive(Debug, Clone)]
pub struct Sessions((Option<MachineSession>, Option<MachineSession>));

#[derive(Debug, Error)]
pub enum SessionError {
    #[error("a session already exists on machine {0}")]
    AlreadyExists(MachineId),
    #[error(
        "{msg}: the session on machine {machine_id} was expected to be in {expected_state}, but actually was in {actual_state}"
    )]
    InvalidState {
        msg: &'static str,
        expected_state: &'static str,
        actual_state: &'static str,
        machine_id: MachineId,
    },
    #[error("there's no session on machine {0}")]
    NoSession(MachineId),
    #[error("on machine {0}: can't evlove to the next state: difficulty is not yet set")]
    NoDifficulty(MachineId),
    #[error("game mode must be single-play or multi-play")]
    UnspecifiedGameMode,
    #[error("multi-play mode is required to do this operation")]
    NotMultiPlay,
    #[error("target {target_id} does not exist on machine {machine_id}")]
    TargetNotFound {
        machine_id: MachineId,
        target_id: Uuid,
    },
}

impl Sessions {
    /// Constructs new [`Sessions`].
    pub fn new() -> Self {
        Self((None, None))
    }

    /// Returns reference to the session with given machine id.
    pub fn get(&self, machine_id: MachineId) -> Option<&MachineSession> {
        if let Some(ms) = &self.0.0
            && ms.machine_id() == machine_id
        {
            Some(ms)
        } else if let Some(ms) = &self.0.1
            && ms.machine_id() == machine_id
        {
            Some(ms)
        } else {
            None
        }
    }

    /// Returns mutable reference to the session with given machine id.
    fn get_mut(&mut self, machine_id: MachineId) -> Option<&mut MachineSession> {
        if let Some(ms) = &mut self.0.0
            && ms.machine_id() == machine_id
        {
            Some(ms)
        } else if let Some(ms) = &mut self.0.1
            && ms.machine_id() == machine_id
        {
            Some(ms)
        } else {
            None
        }
    }

    /// Returns reference to the session whose id is not `machine_id`.
    pub fn get_another(&self, machine_id: MachineId) -> Option<&MachineSession> {
        if let Some(ms) = &self.0.0
            && ms.machine_id() != machine_id
        {
            Some(ms)
        } else if let Some(ms) = &self.0.1
            && ms.machine_id() != machine_id
        {
            Some(ms)
        } else {
            None
        }
    }

    /// Returns mutable reference to the session whose id is not `machine_id`.
    fn get_another_mut(&mut self, machine_id: MachineId) -> Option<&mut MachineSession> {
        if let Some(ms) = &mut self.0.0
            && ms.machine_id() != machine_id
        {
            Some(ms)
        } else if let Some(ms) = &mut self.0.1
            && ms.machine_id() != machine_id
        {
            Some(ms)
        } else {
            None
        }
    }

    /// Inserts new session.
    pub fn insert(
        &mut self,
        machine_id: MachineId,
        player_id: PlayerId,
    ) -> Result<(), SessionError> {
        macro_rules! check_and_insert_to {
            ($another:ident, $field:tt) => {
                if $another.machine_id() == machine_id {
                    Err(SessionError::AlreadyExists(machine_id))
                } else {
                    self.0.$field = Some(MachineSession::ConfiguringGameMode(
                        ConfiguringGameModeState {
                            machine_id,
                            player_id,
                            mode: None,
                        },
                    ));
                    Ok(())
                }
            };
        }
        match (&self.0.0, &self.0.1) {
            (None, None) => {
                self.0.0 = Some(MachineSession::ConfiguringGameMode(
                    ConfiguringGameModeState {
                        machine_id,
                        player_id,
                        mode: None,
                    },
                ));
                Ok(())
            }
            (None, Some(another)) => check_and_insert_to!(another, 0),
            (Some(another), None) => check_and_insert_to!(another, 1),
            (Some(_), Some(_)) => Err(SessionError::AlreadyExists(machine_id)),
        }
    }

    /// Sets the game mode and advances to difficulty configuration so readiness requires a selected mode.
    ///
    /// Accepts [`GameMode::Single`] or [`GameMode::Multi`] for a session in
    /// [`MachineSession::ConfiguringGameMode`], preserving its machine and player IDs.
    /// The difficulty starts unset, and other machine sessions are unchanged.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::UnspecifiedGameMode`] for [`GameMode::Unspecified`],
    /// [`SessionError::NoSession`] if the session does not exist, or
    /// [`SessionError::InvalidState`] if mode selection has already finished.
    /// Errors leave all sessions unchanged.
    pub fn set_game_mode(
        &mut self,
        machine_id: MachineId,
        mode: GameMode,
    ) -> Result<(), SessionError> {
        if mode == GameMode::Unspecified {
            return Err(SessionError::UnspecifiedGameMode);
        }
        let ms = self
            .get_mut(machine_id)
            .ok_or(SessionError::NoSession(machine_id))?;
        let (state,) =
            ms.as_configuring_game_mode()
                .map_err(|actual_state| SessionError::InvalidState {
                    msg: "game mode can only be set while configuring game mode",
                    expected_state: "ConfiguringGameMode",
                    actual_state,
                    machine_id,
                })?;
        let player_id = state.player_id;
        *ms = MachineSession::ConfiguringDifficulty(ConfiguringDifficultyState {
            machine_id,
            player_id,
            mode,
            difficulty: None,
        });
        Ok(())
    }

    /// Updates difficulty.
    ///
    /// If current session on `machine_id` is single-play mode, it updates only that session's difficulty.
    /// If it is multi-play mode, it also updates another session's difficulty.
    pub fn update_difficulty(
        &mut self,
        machine_id: MachineId,
        difficulty: Difficulty,
    ) -> Result<EventTarget, SessionError> {
        let ms = self
            .get_mut(machine_id)
            .ok_or(SessionError::NoSession(machine_id))?;
        let (state,) = ms.as_configuring_difficulty_mut().map_err(|actual_state| {
            SessionError::InvalidState {
                msg: "difficulty can only be updated while configuring difficulty",
                expected_state: "ConfiguringDifficulty",
                actual_state,
                machine_id,
            }
        })?;
        state.difficulty = Some(difficulty);

        if !state.mode.is_multi() {
            return Ok(EventTarget::Single(machine_id));
        }
        let Some(another) = self.get_another_mut(machine_id) else {
            return Ok(EventTarget::All);
        };
        let (state,) = another
            .as_configuring_difficulty_mut()
            .map_err(|actual_state| SessionError::InvalidState {
                msg: "difficulty can only be updated while configuring difficulty",
                expected_state: "ConfiguringDifficulty",
                actual_state,
                machine_id,
            })?;
        debug_assert!(state.mode.is_multi());
        state.difficulty = Some(difficulty);
        Ok(EventTarget::All)
    }

    /// Registers a target in the running game so both multi-play machines can access it.
    pub fn add_target(&mut self, target: Target) -> Result<Uuid, SessionError> {
        let machine_id = target.machine_id.into();
        let ms = self
            .get_mut(machine_id)
            .ok_or(SessionError::NoSession(machine_id))?;
        let (state,) = ms
            .as_running_mut()
            .map_err(|actual_state| SessionError::InvalidState {
                msg: "targets can only be added while running",
                expected_state: "Running",
                actual_state,
                machine_id,
            })?;
        let target_id = Uuid::new_v4();
        state.shared.write().targets.insert(target_id, target);
        Ok(target_id)
    }

    /// Removes a target from the running game, including the other machine in multi-play.
    pub fn remove_target(
        &mut self,
        machine_id: MachineId,
        target_id: Uuid,
    ) -> Result<(), SessionError> {
        let ms = self
            .get_mut(machine_id)
            .ok_or(SessionError::NoSession(machine_id))?;
        let (state,) = ms
            .as_running_mut()
            .map_err(|actual_state| SessionError::InvalidState {
                msg: "targets can only be removed while running",
                expected_state: "Running",
                actual_state,
                machine_id,
            })?;
        state
            .shared
            .write()
            .targets
            .remove(&target_id)
            .ok_or(SessionError::TargetNotFound {
                machine_id,
                target_id,
            })?;
        Ok(())
    }

    /// Returns the score of the session on `machine_id`.
    pub fn score(&self, machine_id: MachineId) -> Result<u32, SessionError> {
        let ms = self
            .get(machine_id)
            .ok_or(SessionError::NoSession(machine_id))?;
        let (state,) = ms
            .as_running()
            .map_err(|actual_state| SessionError::InvalidState {
                msg: "score can only be fetched while running state",
                expected_state: "Running",
                actual_state,
                machine_id,
            })?;
        Ok(state.shared.read().score)
    }

    /// Returns `Ok(score)` when current session is multi-play, otherwise [`SessionError::NotMultiPlay`].
    pub fn score_multi(&self) -> Result<u32, SessionError> {
        let Some(ms1) = &self.0.0 else {
            return Err(SessionError::NotMultiPlay);
        };
        let Some(ms2) = &self.0.1 else {
            return Err(SessionError::NotMultiPlay);
        };
        let (state1,) = ms1
            .as_running()
            .map_err(|actual_state| SessionError::InvalidState {
                msg: "score can only be fetched while Running",
                expected_state: "Running",
                actual_state,
                machine_id: ms1.machine_id(),
            })?;
        let (state2,) = ms2
            .as_running()
            .map_err(|actual_state| SessionError::InvalidState {
                msg: "score can only be fetched while Running",
                expected_state: "Running",
                actual_state,
                machine_id: ms2.machine_id(),
            })?;
        if !Arc::ptr_eq(&state1.shared, &state2.shared) {
            return Err(SessionError::NotMultiPlay);
        }
        let shared = state1.shared.read();
        if !shared.mode.is_multi() {
            return Err(SessionError::NotMultiPlay);
        }
        Ok(shared.score)
    }

    /// Removes the session on `machine_id`, returning the removed session.
    ///
    /// Returns `None` when no session exist on `machine_id`.
    pub fn remove(&mut self, machine_id: MachineId) -> Option<MachineSession> {
        if let Some(ms) = &self.0.0
            && ms.machine_id() == machine_id
        {
            self.0.0.take()
        } else if let Some(ms) = &self.0.1
            && ms.machine_id() == machine_id
        {
            self.0.1.take()
        } else {
            None
        }
    }

    /// Removes all sessions.
    pub fn remove_all(&mut self) -> (Option<MachineSession>, Option<MachineSession>) {
        (self.0.0.take(), self.0.1.take())
    }

    /// Sets the session on `machine_id` ready.
    ///
    /// If the session's mode is [`GameMode::Multi`], it checks if another machine's session is waiting for this session
    /// and then starts a game. If not waiting, it sets the session's state to [`MachineSession::WaitingForAnotherMachine`].
    pub fn set_ready(&mut self, machine_id: MachineId) -> Result<SetReadyAction, SessionError> {
        let ms = self
            .get(machine_id)
            .ok_or(SessionError::NoSession(machine_id))?;
        let (state,) =
            ms.as_configuring_difficulty()
                .map_err(|actual_state| SessionError::InvalidState {
                    msg: "session can be made ready only while ConfiguringDifficulty",
                    expected_state: "ConfiguringDifficulty",
                    actual_state,
                    machine_id,
                })?;
        let player_id = state.player_id;
        let mode = state.mode;
        let difficulty = state
            .difficulty
            .ok_or(SessionError::NoDifficulty(machine_id))?;
        match mode {
            GameMode::Unspecified => unreachable!(),

            GameMode::Single => {
                let ms = self
                    .get_mut(machine_id)
                    .expect("session existence was checked above");
                *ms = MachineSession::Running(RunningState {
                    machine_id,
                    player_id,
                    shared: Arc::new(RwLock::new(SharedRunningState {
                        mode,
                        difficulty,
                        score: 0,
                        targets: HashMap::new(),
                    })),
                });
                Ok(SetReadyAction::RunSingle {
                    player_id,
                    difficulty,
                })
            }
            GameMode::Multi => {
                let Some(another_ms) = self.get_another(machine_id) else {
                    trace!(
                        %machine_id,
                        "no session found for another machine: waiting for it"
                    );
                    let ms = self
                        .get_mut(machine_id)
                        .expect("returned above with SessionError::NoSession");
                    *ms = MachineSession::WaitingForAnotherMachine(WaitingForAnotherMachineState {
                        machine_id,
                        player_id,
                        mode,
                        difficulty,
                    });
                    return Ok(SetReadyAction::WaitForAnother);
                };
                let Ok((another_state,)) = another_ms.as_waiting_for_another_machine() else {
                    trace!(%machine_id, "a session exist on another machine, but it is not waiting for this machine; waiting for it to make session ready");
                    let ms = self
                        .get_mut(machine_id)
                        .expect("returned above with SessionError::NoSession");
                    *ms = MachineSession::WaitingForAnotherMachine(WaitingForAnotherMachineState {
                        machine_id,
                        player_id,
                        mode,
                        difficulty,
                    });
                    return Ok(SetReadyAction::WaitForAnother);
                };
                let another_machine_id = another_state.machine_id;
                let another_player_id = another_state.player_id;
                assert!(another_state.mode.is_multi());
                assert_eq!(difficulty, another_state.difficulty);
                let shared = Arc::new(RwLock::new(SharedRunningState {
                    mode: GameMode::Multi,
                    difficulty,
                    score: 0,
                    targets: HashMap::new(),
                }));
                let action = SetReadyAction::RunMulti {
                    player_ids: [player_id, another_player_id],
                    difficulty: shared.read().difficulty,
                };
                {
                    let ms = self
                        .get_mut(machine_id)
                        .expect("returned with NoSession above");
                    *ms = MachineSession::Running(RunningState {
                        machine_id,
                        player_id,
                        shared: Arc::clone(&shared),
                    });
                }
                {
                    let another_ms = self.get_another_mut(machine_id).expect("returned above");
                    *another_ms = MachineSession::Running(RunningState {
                        machine_id: another_machine_id,
                        player_id: another_player_id,
                        shared,
                    });
                }
                Ok(action)
            }
        }
    }
}

impl Default for Sessions {
    fn default() -> Self {
        Self::new()
    }
}

/// Returned from [`Sessions::set_ready()`].
pub enum SetReadyAction {
    RunSingle {
        player_id: PlayerId,
        difficulty: Difficulty,
    },
    RunMulti {
        player_ids: [PlayerId; 2],
        difficulty: Difficulty,
    },
    WaitForAnother,
}

/// Session data for a specific machine.
#[derive(Debug, Clone, EnumAsInner, EnumAsInnerMut)]
pub enum MachineSession {
    ConfiguringGameMode(ConfiguringGameModeState),
    ConfiguringDifficulty(ConfiguringDifficultyState),
    WaitingForAnotherMachine(WaitingForAnotherMachineState),
    Running(RunningState),
}

/// Keeps mode selection data separate from states that require a selected mode.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfiguringGameModeState {
    pub machine_id: MachineId,
    pub player_id: PlayerId,
    pub mode: Option<GameMode>,
}

/// Keeps difficulty optional until the session is ready to start.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfiguringDifficultyState {
    pub machine_id: MachineId,
    pub player_id: PlayerId,
    pub mode: GameMode,
    pub difficulty: Option<Difficulty>,
}

/// Retains the ready configuration while the other machine prepares its session.
#[derive(Debug, Clone, PartialEq)]
pub struct WaitingForAnotherMachineState {
    pub machine_id: MachineId,
    pub player_id: PlayerId,
    pub mode: GameMode,
    pub difficulty: Difficulty,
}

/// Keeps machine identity local while both multi-play sessions reference the same game.
#[derive(Debug, Clone)]
pub struct RunningState {
    pub machine_id: MachineId,
    pub player_id: PlayerId,
    pub shared: Arc<RwLock<SharedRunningState>>,
}

/// Keeps game configuration, score and targets consistent across multi-play machines.
#[derive(Debug)]
pub struct SharedRunningState {
    pub mode: GameMode,
    pub difficulty: Difficulty,
    pub score: u32,
    pub targets: HashMap<Uuid, Target>,
}

impl MachineSession {
    /// Returns the machine id of this session.
    pub fn machine_id(&self) -> MachineId {
        match self {
            MachineSession::ConfiguringGameMode(state) => state.machine_id,
            MachineSession::ConfiguringDifficulty(state) => state.machine_id,
            MachineSession::WaitingForAnotherMachine(state) => state.machine_id,
            MachineSession::Running(state) => state.machine_id,
        }
    }

    /// Returns the player id of this session.
    #[allow(unused)]
    pub fn player_id(&self) -> PlayerId {
        match self {
            MachineSession::ConfiguringGameMode(state) => state.player_id,
            MachineSession::ConfiguringDifficulty(state) => state.player_id,
            MachineSession::WaitingForAnotherMachine(state) => state.player_id,
            MachineSession::Running(state) => state.player_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::assert_matches;

    use rstest::rstest;

    use super::*;

    fn running_sessions(mode: GameMode) -> Sessions {
        let mut sessions = Sessions::new();
        for id in [1, 2] {
            let machine_id = MachineId::new(id);
            sessions
                .insert(machine_id, PlayerId::new(id))
                .expect("new session");
            sessions
                .set_game_mode(machine_id, mode)
                .expect("initial state");
            sessions
                .update_difficulty(machine_id, Difficulty::Normal)
                .expect("configuring difficulty");
        }
        sessions
            .set_ready(MachineId::new(1))
            .expect("difficulty is set");
        sessions
            .set_ready(MachineId::new(2))
            .expect("difficulty is set");
        sessions
    }

    #[rstest]
    #[case(GameMode::Single)]
    #[case(GameMode::Multi)]
    fn targets_follow_game_sharing(#[case] mode: GameMode) {
        let mut sessions = running_sessions(mode);
        let machine_id = MachineId::new(1);
        let target = Target {
            machine_id: machine_id.into(),
            ..Default::default()
        };
        let first = sessions.add_target(target).expect("running session");
        let second = sessions.add_target(target).expect("running session");
        assert_ne!(first, second);
        assert_eq!(first.get_version_num(), 4);
        let (state,) = sessions
            .get(machine_id)
            .expect("session exists")
            .as_running()
            .expect("running session");
        assert_eq!(state.shared.read().targets.get(&first), Some(&target));
        if mode.is_multi() {
            sessions
                .remove_target(MachineId::new(2), first)
                .expect("multi-play shares targets");
        } else {
            assert_matches!(
                sessions.remove_target(MachineId::new(2), first),
                Err(SessionError::TargetNotFound { .. })
            );
            sessions
                .remove_target(machine_id, first)
                .expect("registered target");
        }
        assert_matches!(
            sessions.remove_target(machine_id, first),
            Err(SessionError::TargetNotFound { .. })
        );
        sessions
            .remove_target(machine_id, second)
            .expect("other target is preserved");
    }

    #[rstest]
    #[case(GameMode::Single)]
    #[case(GameMode::Multi)]
    fn running_game_data_is_shared_only_in_multi_play(#[case] mode: GameMode) {
        let sessions = running_sessions(mode);
        let (first,) = sessions
            .get(MachineId::new(1))
            .expect("first session")
            .as_running()
            .expect("running");
        let (second,) = sessions
            .get(MachineId::new(2))
            .expect("second session")
            .as_running()
            .expect("running");
        assert_eq!(first.machine_id, MachineId::new(1));
        assert_eq!(second.machine_id, MachineId::new(2));
        assert_eq!(first.player_id, PlayerId::new(1));
        assert_eq!(second.player_id, PlayerId::new(2));
        assert_eq!(Arc::ptr_eq(&first.shared, &second.shared), mode.is_multi());
        {
            let mut shared = first.shared.write();
            shared.score = 42;
            shared.difficulty = Difficulty::Hard;
        }
        assert_eq!(sessions.score(MachineId::new(1)).expect("running"), 42);
        if mode.is_multi() {
            assert_eq!(sessions.score(MachineId::new(2)).expect("running"), 42);
            assert_eq!(sessions.score_multi().expect("multi-play"), 42);
            assert_eq!(second.shared.read().difficulty, Difficulty::Hard);
            first.shared.write().mode = GameMode::Single;
            assert_eq!(second.shared.read().mode, GameMode::Single);
        } else {
            assert_eq!(sessions.score(MachineId::new(2)).expect("running"), 0);
            assert_eq!(second.shared.read().difficulty, Difficulty::Normal);
            first.shared.write().mode = GameMode::Multi;
            assert_eq!(second.shared.read().mode, GameMode::Single);
        }
        assert_matches!(sessions.score_multi(), Err(SessionError::NotMultiPlay));
    }

    #[test]
    fn target_operations_require_an_existing_session() {
        let machine_id = MachineId::new(1);
        let target = Target {
            machine_id: machine_id.into(),
            ..Default::default()
        };
        let mut sessions = Sessions::new();
        assert_matches!(sessions.add_target(target), Err(SessionError::NoSession(_)));
        assert_matches!(
            sessions.remove_target(machine_id, Uuid::nil()),
            Err(SessionError::NoSession(_))
        );
    }

    #[rstest]
    #[case::configuring_game_mode(
        MachineSession::ConfiguringGameMode(ConfiguringGameModeState {
            machine_id: MachineId::new(1),
            player_id: PlayerId::new(1),
            mode: None,
        })
    )]
    #[case::configuring_difficulty(
        MachineSession::ConfiguringDifficulty(ConfiguringDifficultyState {
            machine_id: MachineId::new(1),
            player_id: PlayerId::new(1),
            mode: GameMode::Multi,
            difficulty: Some(Difficulty::Normal),
        })
    )]
    #[case::waiting_for_another_machine(
        MachineSession::WaitingForAnotherMachine(WaitingForAnotherMachineState {
            machine_id: MachineId::new(1),
            player_id: PlayerId::new(1),
            mode: GameMode::Multi,
            difficulty: Difficulty::Normal,
        })
    )]
    fn target_operations_require_a_running_session(#[case] session: MachineSession) {
        let machine_id = session.machine_id();
        let target = Target {
            machine_id: machine_id.into(),
            ..Default::default()
        };
        let mut sessions = Sessions((Some(session), None));
        assert_matches!(
            sessions.add_target(target),
            Err(SessionError::InvalidState { .. })
        );
        assert_matches!(
            sessions.remove_target(machine_id, Uuid::nil()),
            Err(SessionError::InvalidState { .. })
        );
    }

    #[test]
    fn sessions_insert_returns_err_when_called_on_same_machine_in_a_row() {
        let machine_id = MachineId::new(1);
        let mut sessions = Sessions::new();
        assert!(sessions.insert(machine_id, PlayerId::new(1)).is_ok());
        let res = sessions.insert(machine_id, PlayerId::new(2));
        assert_matches!(
            res,
            Err(SessionError::AlreadyExists(mid)) if mid == machine_id
        );
    }

    #[rstest]
    #[case::single(GameMode::Single)]
    #[case::multi(GameMode::Multi)]
    fn set_game_mode_advances_only_the_requested_session(#[case] mode: GameMode) {
        let machine_id = MachineId::new(1);
        let player_id = PlayerId::new(2);
        let another_machine_id = MachineId::new(3);
        let mut sessions = Sessions::new();
        sessions
            .insert(another_machine_id, player_id)
            .expect("should succeed");
        sessions
            .insert(machine_id, player_id)
            .expect("should succeed");

        sessions
            .set_game_mode(machine_id, mode)
            .expect("should succeed");

        assert_matches!(
            sessions.get(machine_id),
            Some(MachineSession::ConfiguringDifficulty(ConfiguringDifficultyState {
                machine_id: actual_machine_id,
                player_id: actual_player_id,
                mode: actual_mode,
                difficulty: None,
            })) if *actual_machine_id == machine_id && *actual_player_id == player_id && *actual_mode == mode
        );
        assert_matches!(
            sessions.get(another_machine_id),
            Some(MachineSession::ConfiguringGameMode(
                ConfiguringGameModeState { mode: None, .. }
            ))
        );
    }

    #[test]
    fn set_game_mode_rejects_unspecified_without_changing_state() {
        let machine_id = MachineId::new(1);
        let mut sessions = Sessions::new();
        sessions
            .insert(machine_id, PlayerId::new(2))
            .expect("should succeed");

        assert_matches!(
            sessions.set_game_mode(machine_id, GameMode::Unspecified),
            Err(SessionError::UnspecifiedGameMode)
        );
        assert_matches!(
            sessions.get(machine_id),
            Some(MachineSession::ConfiguringGameMode(
                ConfiguringGameModeState { mode: None, .. }
            ))
        );
    }

    #[test]
    fn set_game_mode_requires_an_existing_session() {
        let machine_id = MachineId::new(1);
        let mut sessions = Sessions::new();
        assert_matches!(
            sessions.set_game_mode(machine_id, GameMode::Single),
            Err(SessionError::NoSession(id)) if id == machine_id
        );
    }

    #[rstest]
    #[case::configuring_difficulty(
        MachineSession::ConfiguringDifficulty(ConfiguringDifficultyState {
            machine_id: MachineId::new(1),
            player_id: PlayerId::new(2),
            mode: GameMode::Multi,
            difficulty: Some(Difficulty::Hard),
        })
    )]
    #[case::waiting_for_another_machine(
        MachineSession::WaitingForAnotherMachine(WaitingForAnotherMachineState {
            machine_id: MachineId::new(1),
            player_id: PlayerId::new(2),
            mode: GameMode::Multi,
            difficulty: Difficulty::Hard,
        })
    )]
    #[case::running(
        MachineSession::Running(RunningState {
            machine_id: MachineId::new(1),
            player_id: PlayerId::new(2),
            shared: Arc::new(RwLock::new(SharedRunningState {
                mode: GameMode::Multi,
                difficulty: Difficulty::Hard,
                score: 10,
                targets: HashMap::new(),
            })),
        })
    )]
    fn set_game_mode_rejects_later_states_without_changing_them(#[case] session: MachineSession) {
        let machine_id = session.machine_id();
        let mut sessions = Sessions((Some(session.clone()), None));
        assert_matches!(
            sessions.set_game_mode(machine_id, GameMode::Single),
            Err(SessionError::InvalidState { machine_id: id, expected_state: "ConfiguringGameMode", .. }) if id == machine_id
        );
        let actual = sessions.get(machine_id).expect("session must still exist");
        match session {
            MachineSession::ConfiguringDifficulty(_) => assert_eq!(
                actual.as_configuring_difficulty(),
                session.as_configuring_difficulty()
            ),
            MachineSession::WaitingForAnotherMachine(_) => assert_eq!(
                actual.as_waiting_for_another_machine(),
                session.as_waiting_for_another_machine()
            ),
            MachineSession::Running(expected) => {
                let (actual,) = actual.as_running().expect("state must remain running");
                assert_eq!(actual.machine_id, expected.machine_id);
                assert_eq!(actual.player_id, expected.player_id);
                assert!(Arc::ptr_eq(&actual.shared, &expected.shared));
                assert_eq!(actual.shared.read().score, 10);
            }
            MachineSession::ConfiguringGameMode(_) => {
                unreachable!("only later states are tested")
            }
        }
    }
}
