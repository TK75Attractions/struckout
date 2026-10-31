//! Session management

use std::{
    collections::{HashMap, HashSet},
    num::NonZeroU32,
    sync::Arc,
};

use parking_lot::RwLock;
use serde::Deserialize;
use struckout_macros::{EnumAsInner, EnumAsInnerMut};
use struckout_proto::{
    Difficulty, GameMode, Target,
    target::{Owner, OwnerSingle, TargetData},
    types::{MachineId, PlayerId},
};
use thiserror::Error;
use time::UtcDateTime;
use tracing::trace;
use uuid::Uuid;

use crate::{GamePlayers, GameRecord, services::EventTarget};

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
    #[error(
        "GameMode must be GAME_MODE_SINGLE or GAME_MODE_MULTI, but GAME_MODE_UNSPECIFIED was given"
    )]
    UnspecifiedGameMode,
    #[error("multi-play mode is required to do this operation: {0}")]
    NotMultiPlay(String),
    #[error("target {target_id} does not exist on machine {machine_id}")]
    TargetNotFound {
        machine_id: MachineId,
        target_id: Uuid,
    },
}

/// Failures that must leave the running game's score unchanged.
#[derive(Debug, Error)]
pub enum HitTargetError {
    #[error(transparent)]
    Session(#[from] SessionError),
    #[error("target_data is required to calculate the score")]
    MissingTargetData,
    #[error("target radius must be finite and positive")]
    InvalidRadius,
    #[error("score exceeds the range of u32")]
    ScoreOverflow,
    #[error("total duration exceeds the range of u32")]
    DurationOverflow,
}

/// A score bonus awarded when a combo reaches its configured count.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComboBonus {
    combo: NonZeroU32,
    points: NonZeroU32,
}

/// Validated bonus rules loaded from the game configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(try_from = "Vec<ComboBonus>")]
pub struct ComboBonuses(Vec<ComboBonus>);

impl TryFrom<Vec<ComboBonus>> for ComboBonuses {
    type Error = String;

    fn try_from(bonuses: Vec<ComboBonus>) -> Result<Self, Self::Error> {
        let mut seen = HashSet::new();
        for bonus in &bonuses {
            if !seen.insert(bonus.combo) {
                return Err(format!("duplicate combo bonus for {}", bonus.combo));
            }
        }
        Ok(Self(bonuses))
    }
}

impl ComboBonuses {
    fn points_for(&self, combo: u32) -> u32 {
        self.0
            .iter()
            .find(|bonus| bonus.combo.get() == combo)
            .map_or(0, |bonus| bonus.points.get())
    }
}

#[derive(Debug, Error)]
pub enum ResetComboError {
    #[error(transparent)]
    Session(#[from] SessionError),
}

/// The score update and optional target replacement produced by a hit.
#[derive(Debug)]
pub struct HitTargetOutcome {
    pub combo: u32,
    pub total_score: u32,
    pub target_points: u32,
    pub combo_bonus: u32,
    pub total_duration_secs: Option<u32>,
    pub event_target: EventTarget,
    pub replacement: Option<(Target, Target)>,
}

/// Creates one pocket watch for each participating machine.
fn pocket_watches(machine_ids: impl IntoIterator<Item = MachineId>) -> HashMap<Uuid, Target> {
    machine_ids
        .into_iter()
        .map(|machine_id| {
            let target_id = Uuid::new_v4();
            (
                target_id,
                Target {
                    target_id: target_id.to_string(),
                    target_data: Some(TargetData::PocketWatch(Default::default())),
                    owner: Some(Owner::Single(OwnerSingle {
                        machine_id: machine_id.into(),
                    })),
                },
            )
        })
        .collect()
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

    /// Registers a target in the running game.
    ///
    /// Returns added [`Target`].
    pub fn add_target(
        &mut self,
        target_data: TargetData,
        owner: Owner,
    ) -> Result<(Uuid, Target), SessionError> {
        let machine_id = match owner {
            Owner::Single(OwnerSingle { machine_id }) => machine_id.into(),
            // take random machine id when multi-play, since runtime state is shared between both machines
            Owner::All(_) => self.machine_ids().ok_or(SessionError::NotMultiPlay(
                "can't add target with Owner::All unless the session is multi-play".into(),
            ))?[0],
        };
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
        let mut shared = state.shared.write();
        let target = Target {
            target_id: target_id.to_string(),
            target_data: Some(target_data),
            owner: Some(owner),
        };
        shared.targets.insert(target_id, target.clone());
        Ok((target_id, target))
    }

    /// Resets consecutive hits, returning the notification target only when changed.
    pub fn reset_combo(
        &mut self,
        machine_id: MachineId,
    ) -> Result<Option<EventTarget>, ResetComboError> {
        let ms = self
            .get(machine_id)
            .ok_or(SessionError::NoSession(machine_id))?;
        let (state,) = ms
            .as_running()
            .map_err(|actual_state| SessionError::InvalidState {
                msg: "combo can only be reset while running",
                expected_state: "Running",
                actual_state,
                machine_id,
            })?;
        let mut shared = state.shared.write();
        if shared.combo == 0 {
            return Ok(None);
        }
        shared.combo = 0;
        Ok(Some(if shared.mode.is_multi() {
            EventTarget::All
        } else {
            EventTarget::Single(machine_id)
        }))
    }

    /// Removes a target from the running game.
    ///
    /// Returns removed [`Target`].
    pub fn remove_target(
        &mut self,
        machine_id: MachineId,
        target_id: Uuid,
    ) -> Result<Target, SessionError> {
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
        let target = state.shared.write().targets.remove(&target_id).ok_or(
            SessionError::TargetNotFound {
                machine_id,
                target_id,
            },
        )?;
        Ok(target)
    }

    /// Adds points to the shared game score and replaces a hit pocket watch.
    ///
    /// Compass hits add `100000 / radius`, truncated to an integer; pocket watch
    /// hits add 500 points. Reaching a combo threshold also adds its bonus.
    /// Errors leave the score, combo and targets unchanged.
    pub fn hit_target(
        &mut self,
        machine_id: MachineId,
        target_id: Uuid,
        combo_bonuses: &ComboBonuses,
        pocket_watch_bonus_secs: NonZeroU32,
    ) -> Result<HitTargetOutcome, HitTargetError> {
        // TODO: スコアの算出を別のモジュールに切り出す
        let ms = self
            .get(machine_id)
            .ok_or(SessionError::NoSession(machine_id))?;
        let (state,) = ms
            .as_running()
            .map_err(|actual_state| SessionError::InvalidState {
                msg: "targets can only be hit while running",
                expected_state: "Running",
                actual_state,
                machine_id,
            })?;
        let mut shared = state.shared.write();
        let target = shared
            .targets
            .get(&target_id)
            .ok_or(SessionError::TargetNotFound {
                machine_id,
                target_id,
            })?;
        let points = match target
            .target_data
            .as_ref()
            .ok_or(HitTargetError::MissingTargetData)?
        {
            TargetData::Compass(compass) => {
                let radius = f64::from(compass.radius);
                if !radius.is_finite() || radius <= 0.0 {
                    return Err(HitTargetError::InvalidRadius);
                }
                (100_000.0 / radius).trunc()
            }
            TargetData::PocketWatch(_) => 500.0,
        };
        if points > f64::from(u32::MAX) {
            return Err(HitTargetError::ScoreOverflow);
        }
        let combo = shared.combo.saturating_add(1);
        let bonus = combo_bonuses.points_for(combo);
        let total_score = shared
            .score
            .checked_add(points as u32)
            .and_then(|score| score.checked_add(bonus))
            .ok_or(HitTargetError::ScoreOverflow)?;
        let total_duration_secs = if matches!(target.target_data, Some(TargetData::PocketWatch(_)))
        {
            Some(
                shared
                    .total_duration_secs
                    .checked_add(pocket_watch_bonus_secs.get())
                    .ok_or(HitTargetError::DurationOverflow)?,
            )
        } else {
            None
        };
        let replacement = if matches!(target.target_data, Some(TargetData::PocketWatch(_))) {
            let removed = target.clone();
            let new_id = Uuid::new_v4();
            let added = Target {
                target_id: new_id.to_string(),
                ..removed.clone()
            };
            shared.targets.remove(&target_id);
            shared.targets.insert(new_id, added.clone());
            Some((removed, added))
        } else {
            None
        };
        shared.score = total_score;
        shared.combo = combo;
        if let Some(total_duration_secs) = total_duration_secs {
            shared.total_duration_secs = total_duration_secs;
        }
        let event_target = if shared.mode.is_multi() {
            EventTarget::All
        } else {
            EventTarget::Single(machine_id)
        };
        Ok(HitTargetOutcome {
            combo,
            total_score,
            target_points: points as u32,
            combo_bonus: bonus,
            total_duration_secs,
            event_target,
            replacement,
        })
    }

    /// Returns the score of the session on `machine_id`.
    #[cfg(test)]
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
    #[cfg(test)]
    pub fn score_multi(&self) -> Result<u32, SessionError> {
        // FIXME: machine_ids()で単純化できる気がする
        let Some(ms1) = &self.0.0 else {
            return Err(SessionError::NotMultiPlay(
                "can't get score without machine_id".into(),
            ));
        };
        let Some(ms2) = &self.0.1 else {
            return Err(SessionError::NotMultiPlay(
                "can't get score without machine_id".into(),
            ));
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
            return Err(SessionError::NotMultiPlay(
                "can't get score without machine_id".into(),
            ));
        }
        let shared = state1.shared.read();
        if !shared.mode.is_multi() {
            return Err(SessionError::NotMultiPlay(
                "can't get score without machine_id".into(),
            ));
        }
        Ok(shared.score)
    }

    /// Freezes the result before notifying clients so no later hits can change it.
    pub fn finish_game(&mut self, machine_id: MachineId) -> Result<(), SessionError> {
        let ms = self
            .get(machine_id)
            .ok_or(SessionError::NoSession(machine_id))?;
        let (state,) = ms
            .as_running()
            .map_err(|actual_state| SessionError::InvalidState {
                msg: "game can only finish while running",
                expected_state: "Running",
                actual_state,
                machine_id,
            })?;
        let (result, is_multi) = {
            let shared = state.shared.read();
            let is_multi = shared.mode.is_multi();
            let players = if is_multi {
                let another = self
                    .get_another(machine_id)
                    .expect("multi-play has two sessions");
                GamePlayers::Multi(
                    (state.machine_id, state.player_id),
                    (another.machine_id(), another.player_id()),
                )
            } else {
                GamePlayers::Single {
                    machine_id,
                    player_id: state.player_id,
                }
            };
            (
                Arc::new(GameRecord {
                    players,
                    started_at: shared.started_at,
                    difficulty: shared.difficulty,
                    score: shared.score,
                }),
                is_multi,
            )
        };
        for ms in self.iter_active_mut() {
            if is_multi || ms.machine_id() == machine_id {
                *ms = MachineSession::Finished(FinishedState {
                    machine_id: ms.machine_id(),
                    player_id: ms.player_id(),
                    result: Arc::clone(&result),
                });
            }
        }
        Ok(())
    }

    /// Reads the frozen result while it is still owned by the session.
    pub fn game_result(&self, machine_id: MachineId) -> Result<&GameRecord, SessionError> {
        let ms = self
            .get(machine_id)
            .ok_or(SessionError::NoSession(machine_id))?;
        match ms {
            MachineSession::Finished(state) => Ok(&state.result),
            _ => Err(SessionError::InvalidState {
                msg: "result is only available after the game finishes",
                expected_state: "Finished",
                actual_state: ms
                    .as_finished()
                    .expect_err("finished states were handled above"),
                machine_id,
            }),
        }
    }

    /// Discards the frozen game after a database save attempt, releasing both multi-play machines.
    pub fn discard_game(&mut self, machine_id: MachineId) -> Result<(), SessionError> {
        if matches!(
            self.game_result(machine_id)?.players,
            GamePlayers::Multi { .. }
        ) {
            self.0 = (None, None);
        } else if self
            .0
            .0
            .as_ref()
            .is_some_and(|ms| ms.machine_id() == machine_id)
        {
            self.0.0 = None;
        } else {
            self.0.1 = None;
        }
        Ok(())
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
                let targets = pocket_watches([machine_id]);
                let initial_targets = targets.values().cloned().collect();
                *ms = MachineSession::Running(RunningState {
                    machine_id,
                    player_id,
                    shared: Arc::new(RwLock::new(SharedRunningState {
                        started_at: UtcDateTime::now(),
                        mode,
                        difficulty,
                        score: 0,
                        combo: 0,
                        total_duration_secs: 0,
                        targets,
                    })),
                });
                Ok(SetReadyAction::RunSingle {
                    initial_targets,
                    difficulty,
                    player_id,
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
                let targets = pocket_watches([machine_id, another_machine_id]);
                let initial_targets = targets.values().cloned().collect();
                let shared = Arc::new(RwLock::new(SharedRunningState {
                    started_at: UtcDateTime::now(),
                    mode: GameMode::Multi,
                    difficulty,
                    score: 0,
                    combo: 0,
                    total_duration_secs: 0,
                    targets,
                }));
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
                let action = SetReadyAction::RunMulti {
                    initial_targets,
                    difficulty,
                    player_1: (machine_id, player_id),
                    player_2: (another_machine_id, another_player_id),
                };
                Ok(action)
            }
        }
    }

    /// Records when the countdown ended and play actually began.
    pub fn mark_started(&mut self, machine_id: MachineId, duration_secs: NonZeroU32) {
        let session = self
            .get(machine_id)
            .expect("countdown session still exists");
        let (running,) = session.as_running().expect("countdown session is running");
        let mut shared = running.shared.write();
        shared.started_at = UtcDateTime::now();
        shared.total_duration_secs = duration_secs.get();
    }

    /// Returns the game duration shared by its timer and participating machines.
    pub fn total_duration_secs(&self, machine_id: MachineId) -> Result<u32, SessionError> {
        let ms = self
            .get(machine_id)
            .ok_or(SessionError::NoSession(machine_id))?;
        let (running,) = ms
            .as_running()
            .map_err(|actual_state| SessionError::InvalidState {
                msg: "total duration is only available while running",
                expected_state: "Running",
                actual_state,
                machine_id,
            })?;
        Ok(running.shared.read().total_duration_secs)
    }

    /// Returns machine ids registered to the session.
    ///
    /// If one of session slot is empty, `None` will be returned regardless of another session slot.
    pub fn machine_ids(&self) -> Option<[MachineId; 2]> {
        let mut id_1 = None;
        let mut id_2 = None;
        if let Some(ms) = &self.0.0 {
            id_1 = Some(ms.machine_id());
        }
        if let Some(ms) = &self.0.1 {
            id_2 = Some(ms.machine_id());
        }
        match (id_1, id_2) {
            (Some(id_1), Some(id_2)) => Some([id_1, id_2]),
            _ => None,
        }
    }

    /// Iterates over active sessions.
    ///
    /// See also [`Self::iter_active_mut()`].
    #[allow(dead_code)] // pair of iter_active_mut()
    fn iter_active(&self) -> impl Iterator<Item = &MachineSession> {
        [&self.0.0, &self.0.1].into_iter().flatten()
    }

    /// Iterates over active sessions.
    ///
    /// Mutable version of [`Self::iter_active()`].
    fn iter_active_mut(&mut self) -> impl Iterator<Item = &mut MachineSession> {
        [&mut self.0.0, &mut self.0.1].into_iter().flatten()
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
        initial_targets: Vec<Target>,
        difficulty: Difficulty,
        player_id: PlayerId,
    },
    RunMulti {
        initial_targets: Vec<Target>,
        difficulty: Difficulty,
        player_1: (MachineId, PlayerId),
        player_2: (MachineId, PlayerId),
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
    Finished(FinishedState),
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

/// Keeps the immutable result while the completed game is being saved.
#[derive(Debug, Clone)]
pub struct FinishedState {
    pub machine_id: MachineId,
    pub player_id: PlayerId,
    pub result: Arc<GameRecord>,
}

/// Keeps game configuration, score and targets consistent across multi-play machines.
#[derive(Debug)]
pub struct SharedRunningState {
    pub started_at: UtcDateTime,
    pub mode: GameMode,
    pub difficulty: Difficulty,
    pub score: u32,
    pub combo: u32,
    pub total_duration_secs: u32,
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
            MachineSession::Finished(state) => state.machine_id,
        }
    }

    /// Returns the player id of this session.
    pub fn player_id(&self) -> PlayerId {
        match self {
            MachineSession::ConfiguringGameMode(state) => state.player_id,
            MachineSession::ConfiguringDifficulty(state) => state.player_id,
            MachineSession::WaitingForAnotherMachine(state) => state.player_id,
            MachineSession::Running(state) => state.player_id,
            MachineSession::Finished(state) => state.player_id,
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

    fn test_combo_bonuses() -> ComboBonuses {
        ComboBonuses::try_from(vec![
            ComboBonus {
                combo: NonZeroU32::new(3).expect("nonzero"),
                points: NonZeroU32::new(500).expect("nonzero"),
            },
            ComboBonus {
                combo: NonZeroU32::new(5).expect("nonzero"),
                points: NonZeroU32::new(800).expect("nonzero"),
            },
        ])
        .expect("unique combos")
    }

    #[rstest]
    #[case(GameMode::Single)]
    #[case(GameMode::Multi)]
    fn targets_follow_game_sharing(#[case] mode: GameMode) {
        let mut sessions = running_sessions(mode);
        let machine_id = MachineId::new(1);
        let target = compass_target(1.0);
        let owner = Owner::Single(OwnerSingle {
            machine_id: machine_id.into(),
        });
        let (first, _) = sessions.add_target(target, owner).expect("running session");
        let (second, _) = sessions.add_target(target, owner).expect("running session");
        assert_ne!(first, second);
        assert_eq!(first.get_version_num(), 4);
        let (state,) = sessions
            .get(machine_id)
            .expect("session exists")
            .as_running()
            .expect("running session");
        let registered = state.shared.read().targets[&first].clone();
        assert_eq!(registered.owner, Some(owner));
        assert_eq!(registered.target_data, Some(target));
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
        assert_matches!(sessions.score_multi(), Err(SessionError::NotMultiPlay(_)));
    }

    fn compass_target(radius: f32) -> TargetData {
        TargetData::Compass(struckout_proto::target::Compass {
            x: 0.0,
            y: 0.0,
            radius,
            vx: 0.0,
            vy: 0.0,
        })
    }

    #[rstest]
    #[case::unit_radius(1.0, 100_000)]
    #[case::larger_radius(200.0, 500)]
    #[case::fractional_points(3.0, 33_333)]
    #[case::less_than_one_point(200_000.0, 0)]
    fn hit_target_adds_points_for_each_hit(#[case] radius: f32, #[case] points: u32) {
        let mut sessions = running_sessions(GameMode::Single);
        let machine_id = MachineId::new(1);
        let target_id = sessions
            .add_target(
                compass_target(radius),
                Owner::Single(OwnerSingle {
                    machine_id: machine_id.into(),
                }),
            )
            .expect("running game")
            .0;
        let HitTargetOutcome {
            total_score: score,
            event_target: target,
            ..
        } = sessions
            .hit_target(
                machine_id,
                target_id,
                &test_combo_bonuses(),
                NonZeroU32::new(5).expect("nonzero"),
            )
            .expect("registered target");
        assert_eq!(score, points);
        assert_matches!(target, EventTarget::Single(id) if id == machine_id);
        let HitTargetOutcome {
            total_score: score, ..
        } = sessions
            .hit_target(
                machine_id,
                target_id,
                &test_combo_bonuses(),
                NonZeroU32::new(5).expect("nonzero"),
            )
            .expect("target remains registered");
        assert_eq!(score, 2 * points);
        assert_eq!(sessions.score(machine_id).expect("running"), 2 * points);
        assert_eq!(
            sessions.score(MachineId::new(2)).expect("independent game"),
            0
        );
    }

    #[test]
    fn hit_target_updates_the_shared_multi_play_score() {
        let mut sessions = running_sessions(GameMode::Multi);
        let target_id = sessions
            .add_target(compass_target(200.0), Owner::All(Default::default()))
            .expect("running game")
            .0;
        let HitTargetOutcome {
            total_score: score,
            event_target: target,
            ..
        } = sessions
            .hit_target(
                MachineId::new(2),
                target_id,
                &test_combo_bonuses(),
                NonZeroU32::new(5).expect("nonzero"),
            )
            .expect("shared target");
        assert_eq!(score, 500);
        assert_matches!(target, EventTarget::All);
        assert_eq!(
            sessions.score(MachineId::new(1)).expect("first machine"),
            500
        );
        assert_eq!(
            sessions.score(MachineId::new(2)).expect("second machine"),
            500
        );
        assert_eq!(sessions.score_multi().expect("shared score"), 500);
    }

    #[test]
    fn configured_combo_bonuses_apply_at_exact_counts_and_after_reset() {
        let bonuses = ComboBonuses::try_from(vec![ComboBonus {
            combo: NonZeroU32::new(2).expect("nonzero"),
            points: NonZeroU32::new(75).expect("nonzero"),
        }])
        .expect("unique combo");
        let mut sessions = running_sessions(GameMode::Single);
        let machine_id = MachineId::new(1);
        let target_id = sessions
            .add_target(
                compass_target(200.0),
                Owner::Single(OwnerSingle {
                    machine_id: machine_id.into(),
                }),
            )
            .expect("running game")
            .0;

        for expected_score in [500, 1075, 1575] {
            assert_eq!(
                sessions
                    .hit_target(
                        machine_id,
                        target_id,
                        &bonuses,
                        NonZeroU32::new(5).expect("nonzero")
                    )
                    .expect("registered target")
                    .total_score,
                expected_score
            );
        }
        sessions.reset_combo(machine_id).expect("running game");
        for expected_score in [2075, 2650] {
            assert_eq!(
                sessions
                    .hit_target(
                        machine_id,
                        target_id,
                        &bonuses,
                        NonZeroU32::new(5).expect("nonzero")
                    )
                    .expect("registered target")
                    .total_score,
                expected_score
            );
        }
    }

    #[test]
    fn combo_bonus_overflow_preserves_score_combo_and_targets() {
        let mut sessions = running_sessions(GameMode::Single);
        let machine_id = MachineId::new(1);
        let (target_id, targets) = {
            let (state,) = sessions
                .get(machine_id)
                .expect("session")
                .as_running()
                .expect("running");
            let mut shared = state.shared.write();
            shared.combo = 2;
            shared.score = u32::MAX - 500;
            (
                *shared.targets.keys().next().expect("initial watch"),
                shared.targets.clone(),
            )
        };
        assert_matches!(
            sessions.hit_target(
                machine_id,
                target_id,
                &test_combo_bonuses(),
                NonZeroU32::new(5).expect("nonzero")
            ),
            Err(HitTargetError::ScoreOverflow)
        );
        let (state,) = sessions
            .get(machine_id)
            .expect("session")
            .as_running()
            .expect("running");
        let shared = state.shared.read();
        assert_eq!(shared.combo, 2);
        assert_eq!(shared.score, u32::MAX - 500);
        assert_eq!(shared.targets, targets);
    }

    #[rstest]
    #[case::zero(0.0)]
    #[case::negative(-1.0)]
    #[case::nan(f32::NAN)]
    #[case::infinite(f32::INFINITY)]
    #[case::negative_infinite(f32::NEG_INFINITY)]
    fn hit_target_rejects_invalid_radii(#[case] radius: f32) {
        let mut sessions = running_sessions(GameMode::Single);
        let machine_id = MachineId::new(1);
        let target_id = sessions
            .add_target(
                compass_target(radius),
                Owner::Single(OwnerSingle {
                    machine_id: machine_id.into(),
                }),
            )
            .expect("running game")
            .0;
        assert_matches!(
            sessions.hit_target(
                machine_id,
                target_id,
                &test_combo_bonuses(),
                NonZeroU32::new(5).expect("nonzero")
            ),
            Err(HitTargetError::InvalidRadius)
        );
        assert_eq!(sessions.score(machine_id).expect("running"), 0);
    }

    #[rstest]
    #[case::points_overflow(f32::MIN_POSITIVE, 0)]
    #[case::total_overflow(1.0, u32::MAX)]
    fn hit_target_rejects_score_overflow(#[case] radius: f32, #[case] score: u32) {
        let mut sessions = running_sessions(GameMode::Single);
        let machine_id = MachineId::new(1);
        let target_id = sessions
            .add_target(
                compass_target(radius),
                Owner::Single(OwnerSingle {
                    machine_id: machine_id.into(),
                }),
            )
            .expect("running game")
            .0;
        let (state,) = sessions
            .get(machine_id)
            .expect("session")
            .as_running()
            .expect("running");
        state.shared.write().score = score;
        assert_matches!(
            sessions.hit_target(
                machine_id,
                target_id,
                &test_combo_bonuses(),
                NonZeroU32::new(5).expect("nonzero")
            ),
            Err(HitTargetError::ScoreOverflow)
        );
        assert_eq!(sessions.score(machine_id).expect("running"), score);
    }

    #[test]
    fn pocket_watch_score_overflow_preserves_the_target_and_start_time() {
        let mut sessions = running_sessions(GameMode::Multi);
        let machine_id = MachineId::new(1);
        let (target_id, targets, started_at) = {
            let (state,) = sessions
                .get(machine_id)
                .expect("session")
                .as_running()
                .expect("running");
            let mut shared = state.shared.write();
            shared.score = u32::MAX - 499;
            let target_id = *shared.targets.keys().next().expect("initial pocket watch");
            (target_id, shared.targets.clone(), shared.started_at)
        };
        assert_matches!(
            sessions.hit_target(
                machine_id,
                target_id,
                &test_combo_bonuses(),
                NonZeroU32::new(5).expect("nonzero")
            ),
            Err(HitTargetError::ScoreOverflow)
        );
        let (state,) = sessions
            .get(machine_id)
            .expect("session")
            .as_running()
            .expect("running");
        let shared = state.shared.read();
        assert_eq!(shared.score, u32::MAX - 499);
        assert_eq!(shared.targets, targets);
        assert_eq!(shared.started_at, started_at);
    }

    #[test]
    fn pocket_watch_duration_overflow_preserves_score_combo_and_target() {
        let mut sessions = running_sessions(GameMode::Single);
        let machine_id = MachineId::new(1);
        let (target_id, targets) = {
            let (state,) = sessions
                .get(machine_id)
                .expect("session")
                .as_running()
                .expect("running");
            let mut shared = state.shared.write();
            shared.total_duration_secs = u32::MAX - 4;
            (
                *shared.targets.keys().next().expect("watch"),
                shared.targets.clone(),
            )
        };
        assert_matches!(
            sessions.hit_target(
                machine_id,
                target_id,
                &test_combo_bonuses(),
                NonZeroU32::new(5).expect("nonzero")
            ),
            Err(HitTargetError::DurationOverflow)
        );
        let (state,) = sessions
            .get(machine_id)
            .expect("session")
            .as_running()
            .expect("running");
        let shared = state.shared.read();
        assert_eq!(shared.score, 0);
        assert_eq!(shared.combo, 0);
        assert_eq!(shared.total_duration_secs, u32::MAX - 4);
        assert_eq!(shared.targets, targets);
    }

    #[test]
    fn hit_target_rejects_missing_target_data() {
        let mut sessions = running_sessions(GameMode::Single);
        let machine_id = MachineId::new(1);
        let target_id = sessions
            .add_target(
                compass_target(1.0),
                Owner::Single(OwnerSingle {
                    machine_id: machine_id.into(),
                }),
            )
            .expect("running game")
            .0;
        let (state,) = sessions.get(machine_id).unwrap().as_running().unwrap();
        state
            .shared
            .write()
            .targets
            .get_mut(&target_id)
            .unwrap()
            .target_data = None;
        assert_matches!(
            sessions.hit_target(
                machine_id,
                target_id,
                &test_combo_bonuses(),
                NonZeroU32::new(5).expect("nonzero")
            ),
            Err(HitTargetError::MissingTargetData)
        );
        assert_eq!(sessions.score(machine_id).expect("running"), 0);
    }

    #[test]
    fn target_operations_require_an_existing_session() {
        let machine_id = MachineId::new(1);
        let target = compass_target(1.0);
        let owner = Owner::Single(OwnerSingle {
            machine_id: machine_id.into(),
        });
        let mut sessions = Sessions::new();
        assert_matches!(
            sessions.add_target(target, owner),
            Err(SessionError::NoSession(_))
        );
        assert_matches!(
            sessions.hit_target(
                machine_id,
                Uuid::nil(),
                &test_combo_bonuses(),
                NonZeroU32::new(5).expect("nonzero")
            ),
            Err(HitTargetError::Session(SessionError::NoSession(_)))
        );
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
        let target = compass_target(1.0);
        let owner = Owner::Single(OwnerSingle {
            machine_id: machine_id.into(),
        });
        let mut sessions = Sessions((Some(session), None));
        assert_matches!(
            sessions.hit_target(
                machine_id,
                Uuid::nil(),
                &test_combo_bonuses(),
                NonZeroU32::new(5).expect("nonzero")
            ),
            Err(HitTargetError::Session(SessionError::InvalidState { .. }))
        );
        assert_matches!(
            sessions.add_target(target, owner),
            Err(SessionError::InvalidState { .. })
        );
        assert_matches!(
            sessions.remove_target(machine_id, Uuid::nil()),
            Err(SessionError::InvalidState { .. })
        );
    }

    #[test]
    fn completing_single_play_preserves_the_other_machine() {
        let mut sessions = running_sessions(GameMode::Single);
        let first = MachineId::new(1);
        let second = MachineId::new(2);
        let (state,) = sessions.get(first).unwrap().as_running().unwrap();
        let started_at = state.shared.read().started_at;
        state.shared.write().score = 123;
        sessions.finish_game(first).expect("first game is running");
        assert!(sessions.get(second).unwrap().as_running().is_ok());
        sessions
            .finish_game(second)
            .expect("second game is running");
        let game = sessions.game_result(first).expect("finished game").clone();
        sessions.discard_game(first).expect("discard saved game");
        assert_eq!(game.score, 123);
        assert_eq!(game.started_at, started_at);
        assert!(sessions.get(first).is_none());
        assert_eq!(sessions.game_result(second).unwrap().score, 0);
        sessions
            .insert(first, PlayerId::new(3))
            .expect("a new session can start on the freed machine");
        sessions
            .discard_game(second)
            .expect("second game can be discarded independently");
        assert!(
            sessions
                .get(first)
                .unwrap()
                .as_configuring_game_mode()
                .is_ok()
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
                started_at: UtcDateTime::now(),
                mode: GameMode::Multi,
                difficulty: Difficulty::Hard,
                score: 10,
                combo: 0,
                total_duration_secs: 90,
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
            MachineSession::Finished(expected) => {
                assert_eq!(actual.machine_id(), expected.machine_id);
                assert_eq!(actual.player_id(), expected.player_id);
                assert_eq!(
                    sessions.game_result(machine_id).unwrap(),
                    expected.result.as_ref()
                );
            }
            MachineSession::ConfiguringGameMode(_) => {
                unreachable!("only later states are tested")
            }
        }
    }
}
