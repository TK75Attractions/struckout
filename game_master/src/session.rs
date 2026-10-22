//! Session management

use struckout_macros::{EnumAsInner, EnumAsInnerMut};
use struckout_proto::{
    Difficulty, GameMode,
    types::{MachineId, PlayerId},
};
use thiserror::Error;

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
}

impl Sessions {
    /// Constructs new [`Sessions`].
    pub fn new() -> Self {
        Self((None, None))
    }

    /// Returns whether there is a session with multiplay mode.
    pub fn includes_multiplay(&self) -> bool {
        todo!()
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

    /// Updates difficulty.
    ///
    /// If current session on `machine_id` is single-play mode, it updates only that session's difficulty.
    /// If it is multi-play mode, it also updates another session's difficulty.
    pub fn update_difficulty(
        &mut self,
        machine_id: MachineId,
        difficulty: Difficulty,
    ) -> Result<(), SessionError> {
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
            return Ok(());
        }
        let Some(another) = self.get_another_mut(machine_id) else {
            return Ok(());
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
        Ok(())
    }

    pub fn score(&self, machine_id: MachineId) -> Result<u32, SessionError> {
        let ms = self
            .get(machine_id)
            .ok_or(SessionError::NoSession(machine_id))?;
        let (_, _, _, _, score) =
            ms.as_running()
                .map_err(|actual_state| SessionError::InvalidState {
                    msg: "score can only be fetched while running state",
                    expected_state: "Running",
                    actual_state,
                    machine_id,
                })?;
        Ok(*score)
    }
}

impl Default for Sessions {
    fn default() -> Self {
        Self::new()
    }
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
    Running {
        machine_id: MachineId,
        player_id: PlayerId,
        mode: GameMode,
        difficulty: Difficulty,
        score: u32,
    },
}

impl MachineSession {
    /// Returns the machine id of this session.
    pub fn machine_id(&self) -> MachineId {
        match self {
            MachineSession::ConfiguringGameMode { machine_id, .. } => *machine_id,
            MachineSession::ConfiguringDifficulty { machine_id, .. } => *machine_id,
            MachineSession::Running { machine_id, .. } => *machine_id,
        }
    }

    /// Returns the player id of this session.
    pub fn player_id(&self) -> PlayerId {
        match self {
            MachineSession::ConfiguringGameMode { player_id, .. } => *player_id,
            MachineSession::ConfiguringDifficulty { player_id, .. } => *player_id,
            MachineSession::Running { player_id, .. } => *player_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::assert_matches;

    use super::*;

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
    fn sessions_update_difficulty_returns_err_when_called_in_configuring_mode() {
        let mut sessions = Sessions::new();
        todo!()
    }
}
