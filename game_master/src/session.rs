//! Session management

use std::collections::HashMap;

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
                    self.0.$field = Some(MachineSession::ConfiguringGameMode {
                        machine_id,
                        player_id,
                        mode: None,
                    });
                    Ok(())
                }
            };
        }
        match (&self.0.0, &self.0.1) {
            (None, None) => {
                self.0.0 = Some(MachineSession::ConfiguringGameMode {
                    machine_id,
                    player_id,
                    mode: None,
                });
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
        let (_, &player_id, _) =
            ms.as_configuring_game_mode()
                .map_err(|actual_state| SessionError::InvalidState {
                    msg: "game mode can only be set while configuring game mode",
                    expected_state: "ConfiguringGameMode",
                    actual_state,
                    machine_id,
                })?;
        *ms = MachineSession::ConfiguringDifficulty {
            machine_id,
            player_id,
            mode,
            difficulty: None,
        };
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
        let (_, _, mode, di) = ms.as_configuring_difficulty_mut().map_err(|actual_state| {
            SessionError::InvalidState {
                msg: "difficulty can only be updated while configuring difficulty".into(),
                expected_state: "ConfiguringDifficulty".into(),
                actual_state,
                machine_id,
            }
        })?;
        *di = Some(difficulty);

        if !mode.is_multi() {
            return Ok(EventTarget::Single(machine_id));
        }
        let Some(another) = self.get_another_mut(machine_id) else {
            return Ok(EventTarget::All);
        };
        let (_, _, mode, di) = another
            .as_configuring_difficulty_mut()
            .map_err(|actual_state| SessionError::InvalidState {
                msg: "difficulty can only be updated while configuring difficulty".into(),
                expected_state: "ConfiguringDifficulty",
                actual_state,
                machine_id,
            })?;
        debug_assert!(mode.is_multi());
        *di = Some(difficulty);
        Ok(EventTarget::All)
    }

    /// Registers a target in its running session so it is discarded when the session ends.
    pub fn add_target(&mut self, target: Target) -> Result<Uuid, SessionError> {
        let machine_id = target.machine_id.into();
        let ms = self
            .get_mut(machine_id)
            .ok_or(SessionError::NoSession(machine_id))?;
        let (_, _, _, _, _, targets) =
            ms.as_running_mut()
                .map_err(|actual_state| SessionError::InvalidState {
                    msg: "targets can only be added while running",
                    expected_state: "Running",
                    actual_state,
                    machine_id,
                })?;
        let target_id = Uuid::new_v4();
        targets.insert(target_id, target);
        Ok(target_id)
    }

    /// Removes a target only from the requested running session to keep machines isolated.
    pub fn remove_target(
        &mut self,
        machine_id: MachineId,
        target_id: Uuid,
    ) -> Result<(), SessionError> {
        let ms = self
            .get_mut(machine_id)
            .ok_or(SessionError::NoSession(machine_id))?;
        let (_, _, _, _, _, targets) =
            ms.as_running_mut()
                .map_err(|actual_state| SessionError::InvalidState {
                    msg: "targets can only be removed while running",
                    expected_state: "Running",
                    actual_state,
                    machine_id,
                })?;
        targets
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
        let (_, _, _, _, score, _) =
            ms.as_running()
                .map_err(|actual_state| SessionError::InvalidState {
                    msg: "score can only be fetched while running state",
                    expected_state: "Running",
                    actual_state,
                    machine_id,
                })?;
        Ok(*score)
    }

    /// Returns `Ok(score)` when current session is multi-play, otherwise [`SessionError::NotMultiPlay`].
    pub fn score_multi(&self) -> Result<u32, SessionError> {
        let Some(ms1) = &self.0.0 else {
            return Err(SessionError::NotMultiPlay);
        };
        let Some(ms2) = &self.0.1 else {
            return Err(SessionError::NotMultiPlay);
        };
        let (_, _, mode1, dfclty1, score1, _) =
            ms1.as_running()
                .map_err(|actual_state| SessionError::InvalidState {
                    msg: "score can only be fetched while Running",
                    expected_state: "Running",
                    actual_state,
                    machine_id: ms1.machine_id(),
                })?;
        let (_, _, mode2, dfclty2, score2, _) =
            ms2.as_running()
                .map_err(|actual_state| SessionError::InvalidState {
                    msg: "score can only be fetched while Running",
                    expected_state: "Running",
                    actual_state,
                    machine_id: ms2.machine_id(),
                })?;
        if !mode1.is_multi() {
            return Err(SessionError::NotMultiPlay);
        }
        if !mode2.is_multi() {
            return Err(SessionError::NotMultiPlay);
        }
        assert_eq!(dfclty1, dfclty2);
        assert_eq!(score1, score2);
        Ok(*score1)
    }

    /// Removes the session on `machine_id`, returning the removed session.
    ///
    /// Returns `None` when no session exist on `machine_id`.
    pub fn remove(&mut self, machine_id: MachineId) -> Option<MachineSession> {
        if let Some(ms) = &self.0.0
            && ms.machine_id() == machine_id
        {
            std::mem::replace(&mut self.0.0, None)
        } else if let Some(ms) = &self.0.1
            && ms.machine_id() == machine_id
        {
            std::mem::replace(&mut self.0.0, None)
        } else {
            None
        }
    }

    /// Removes all sessions.
    pub fn remove_all(&mut self) -> (Option<MachineSession>, Option<MachineSession>) {
        (
            std::mem::replace(&mut self.0.0, None),
            std::mem::replace(&mut self.0.1, None),
        )
    }

    /// Sets the session on `machine_id` ready.
    ///
    /// If the session's mode is [`GameMode::Multi`], it checks if another machine's session is waiting for this session
    /// and then starts a game. If not waiting, it sets the session's state to [`MachineSession::WaitingForAnotherMachine`].
    pub fn set_ready(&mut self, machine_id: MachineId) -> Result<SetReadyAction, SessionError> {
        let ms = self
            .get(machine_id)
            .ok_or(SessionError::NoSession(machine_id))?;
        let (_, &player_id, &mode, &difficulty) =
            ms.as_configuring_difficulty()
                .map_err(|actual_state| SessionError::InvalidState {
                    msg: "session can be made ready only while ConfiguringDifficulty",
                    expected_state: "ConfiguringDifficulty",
                    actual_state,
                    machine_id,
                })?;
        let difficulty = difficulty.ok_or(SessionError::NoDifficulty(machine_id))?;
        match mode {
            GameMode::Unspecified => unreachable!(),

            GameMode::Single => {
                let ms = self
                    .get_mut(machine_id)
                    .expect("session existence was checked above");
                *ms = MachineSession::Running {
                    machine_id,
                    player_id,
                    mode,
                    difficulty,
                    score: 0,
                    targets: HashMap::new(),
                };
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
                    *ms = MachineSession::WaitingForAnotherMachine {
                        machine_id,
                        player_id,
                        mode,
                        difficulty,
                    };
                    return Ok(SetReadyAction::WaitForAnother);
                };
                let Ok((&another_machine_id, &another_player_id, &mode, &another_difficulty)) =
                    another_ms.as_waiting_for_another_machine()
                else {
                    trace!(%machine_id, "a session exist on another machine, but it is not waiting for this machine; waiting for it to make session ready");
                    let ms = self
                        .get_mut(machine_id)
                        .expect("returned above with SessionError::NoSession");
                    *ms = MachineSession::WaitingForAnotherMachine {
                        machine_id,
                        player_id,
                        mode,
                        difficulty,
                    };
                    return Ok(SetReadyAction::WaitForAnother);
                };
                assert!(mode.is_multi());
                assert_eq!(difficulty, another_difficulty);
                {
                    let ms = self
                        .get_mut(machine_id)
                        .expect("returned with NoSession above");
                    *ms = MachineSession::Running {
                        machine_id,
                        player_id,
                        mode: GameMode::Multi,
                        difficulty,
                        score: 0,
                        targets: HashMap::new(),
                    };
                }
                {
                    let another_ms = self.get_another_mut(machine_id).expect("returned above");
                    *another_ms = MachineSession::Running {
                        machine_id: another_machine_id,
                        player_id: another_player_id,
                        mode: GameMode::Multi,
                        difficulty,
                        score: 0,
                        targets: HashMap::new(),
                    };
                }
                Ok(SetReadyAction::RunMulti {
                    player_ids: [player_id, another_player_id],
                    difficulty,
                })
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
    ConfiguringGameMode {
        machine_id: MachineId,
        player_id: PlayerId,
        mode: Option<GameMode>,
    },
    ConfiguringDifficulty {
        machine_id: MachineId,
        player_id: PlayerId,
        mode: GameMode,
        difficulty: Option<Difficulty>,
    },
    WaitingForAnotherMachine {
        machine_id: MachineId,
        player_id: PlayerId,
        mode: GameMode,
        difficulty: Difficulty,
    },
    Running {
        machine_id: MachineId,
        player_id: PlayerId,
        mode: GameMode,
        difficulty: Difficulty,
        score: u32,
        targets: HashMap<Uuid, Target>,
    },
}

impl MachineSession {
    /// Returns the machine id of this session.
    pub fn machine_id(&self) -> MachineId {
        match self {
            MachineSession::ConfiguringGameMode { machine_id, .. } => *machine_id,
            MachineSession::ConfiguringDifficulty { machine_id, .. } => *machine_id,
            MachineSession::WaitingForAnotherMachine { machine_id, .. } => *machine_id,
            MachineSession::Running { machine_id, .. } => *machine_id,
        }
    }

    /// Returns the player id of this session.
    #[allow(unused)]
    pub fn player_id(&self) -> PlayerId {
        match self {
            MachineSession::ConfiguringGameMode { player_id, .. } => *player_id,
            MachineSession::ConfiguringDifficulty { player_id, .. } => *player_id,
            MachineSession::WaitingForAnotherMachine { player_id, .. } => *player_id,
            MachineSession::Running { player_id, .. } => *player_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::assert_matches;

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

    #[rstest::rstest]
    #[case(GameMode::Single)]
    #[case(GameMode::Multi)]
    fn targets_are_stored_and_removed_per_machine(#[case] mode: GameMode) {
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
        let (_, _, _, _, _, targets) = sessions
            .get(machine_id)
            .expect("session exists")
            .as_running()
            .expect("running session");
        assert_eq!(targets.get(&first), Some(&target));
        assert_matches!(
            sessions.remove_target(MachineId::new(2), first),
            Err(SessionError::TargetNotFound { .. })
        );
        sessions
            .remove_target(machine_id, first)
            .expect("registered target");
        assert_matches!(
            sessions.remove_target(machine_id, first),
            Err(SessionError::TargetNotFound { .. })
        );
        sessions
            .remove_target(machine_id, second)
            .expect("other target is preserved");
    }

    #[test]
    fn target_operations_require_a_running_session() {
        let machine_id = MachineId::new(1);
        let player_id = PlayerId::new(1);
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
        for session in [
            MachineSession::ConfiguringGameMode {
                machine_id,
                player_id,
                mode: None,
            },
            MachineSession::ConfiguringDifficulty {
                machine_id,
                player_id,
                mode: GameMode::Multi,
                difficulty: Some(Difficulty::Normal),
            },
            MachineSession::WaitingForAnotherMachine {
                machine_id,
                player_id,
                mode: GameMode::Multi,
                difficulty: Difficulty::Normal,
            },
        ] {
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

    #[test]
    fn set_game_mode_advances_only_the_requested_session() {
        let machine_id = MachineId::new(1);
        let player_id = PlayerId::new(2);
        let another_machine_id = MachineId::new(3);
        for mode in [GameMode::Single, GameMode::Multi] {
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
                Some(MachineSession::ConfiguringDifficulty {
                    machine_id: actual_machine_id,
                    player_id: actual_player_id,
                    mode: actual_mode,
                    difficulty: None,
                }) if *actual_machine_id == machine_id && *actual_player_id == player_id && *actual_mode == mode
            );
            assert_matches!(
                sessions.get(another_machine_id),
                Some(MachineSession::ConfiguringGameMode { mode: None, .. })
            );
        }
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
            Some(MachineSession::ConfiguringGameMode { mode: None, .. })
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

    #[test]
    fn set_game_mode_rejects_later_states_without_changing_them() {
        let machine_id = MachineId::new(1);
        let player_id = PlayerId::new(2);
        let mode = GameMode::Multi;
        let difficulty = Difficulty::Hard;
        for session in [
            MachineSession::ConfiguringDifficulty {
                machine_id,
                player_id,
                mode,
                difficulty: Some(difficulty),
            },
            MachineSession::WaitingForAnotherMachine {
                machine_id,
                player_id,
                mode,
                difficulty,
            },
            MachineSession::Running {
                machine_id,
                player_id,
                mode,
                difficulty,
                score: 10,
                targets: HashMap::new(),
            },
        ] {
            let mut sessions = Sessions((Some(session.clone()), None));
            assert_matches!(
                sessions.set_game_mode(machine_id, GameMode::Single),
                Err(SessionError::InvalidState { machine_id: id, expected_state: "ConfiguringGameMode", .. }) if id == machine_id
            );
            let actual = sessions.get(machine_id).expect("session must still exist");
            match session {
                MachineSession::ConfiguringDifficulty { .. } => assert_eq!(
                    actual.as_configuring_difficulty(),
                    session.as_configuring_difficulty()
                ),
                MachineSession::WaitingForAnotherMachine { .. } => assert_eq!(
                    actual.as_waiting_for_another_machine(),
                    session.as_waiting_for_another_machine()
                ),
                MachineSession::Running { .. } => {
                    assert_eq!(actual.as_running(), session.as_running())
                }
                MachineSession::ConfiguringGameMode { .. } => {
                    unreachable!("only later states are tested")
                }
            }
        }
    }
}
