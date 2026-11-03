//! Session state stays in one place so stale timers cannot modify a later game.

use struckout_proto::{
    Difficulty,
    types::{GameId, MachineId, PlayerId},
};
use thiserror::Error;
use tokio::time::Instant;

#[derive(Debug, Clone, Default)]
pub(crate) struct Sessions((Option<MachineSession>, Option<MachineSession>));

#[derive(Debug, Error)]
pub(crate) enum SessionError {
    #[error("a session already exists on machine {0}")]
    AlreadyExists(MachineId),
    #[error("both session slots are occupied")]
    Capacity,
    #[error("there's no session on machine {0}")]
    NoSession(MachineId),
    #[error("{operation} is invalid while machine {machine_id} is in {state}")]
    InvalidState {
        operation: &'static str,
        machine_id: MachineId,
        state: &'static str,
    },
    #[error("difficulty has not been selected for machine {0}")]
    MissingDifficulty(MachineId),
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ReadyConfiguration {
    pub machine_id: MachineId,
    pub player_id: PlayerId,
    pub difficulty: Difficulty,
}

impl Sessions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, machine_id: MachineId) -> Option<&MachineSession> {
        [&self.0.0, &self.0.1]
            .into_iter()
            .flatten()
            .find(|s| s.machine_id() == machine_id)
    }

    pub fn get_mut(&mut self, machine_id: MachineId) -> Option<&mut MachineSession> {
        [&mut self.0.0, &mut self.0.1]
            .into_iter()
            .flatten()
            .find(|s| s.machine_id() == machine_id)
    }

    /// Retry configuration for the same player without replacing a running game.
    pub fn insert(
        &mut self,
        machine_id: MachineId,
        player_id: PlayerId,
    ) -> Result<(), SessionError> {
        if let Some(session) = self.get_mut(machine_id) {
            if session.player_id() != player_id || !session.is_configuring() {
                return Err(SessionError::AlreadyExists(machine_id));
            }
            *session = MachineSession::ConfiguringGameMode {
                machine_id,
                player_id,
            };
            return Ok(());
        }
        let slot = if self.0.0.is_none() {
            &mut self.0.0
        } else if self.0.1.is_none() {
            &mut self.0.1
        } else {
            return Err(SessionError::Capacity);
        };
        *slot = Some(MachineSession::ConfiguringGameMode {
            machine_id,
            player_id,
        });
        Ok(())
    }

    pub fn set_single_mode(&mut self, machine_id: MachineId) -> Result<(), SessionError> {
        let session = self
            .get_mut(machine_id)
            .ok_or(SessionError::NoSession(machine_id))?;
        if !session.is_configuring() {
            return Err(session.invalid("set game mode"));
        }
        *session = MachineSession::ConfiguringDifficulty {
            machine_id,
            player_id: session.player_id(),
            difficulty: None,
        };
        Ok(())
    }

    pub fn update_difficulty(
        &mut self,
        machine_id: MachineId,
        difficulty: Difficulty,
    ) -> Result<(), SessionError> {
        let session = self
            .get_mut(machine_id)
            .ok_or(SessionError::NoSession(machine_id))?;
        match session {
            MachineSession::ConfiguringDifficulty {
                difficulty: value, ..
            } => {
                *value = Some(difficulty);
                Ok(())
            }
            _ => Err(session.invalid("set difficulty")),
        }
    }

    /// Reserve before the database await so concurrent Ready requests cannot insert two games.
    pub fn reserve_start(
        &mut self,
        machine_id: MachineId,
    ) -> Result<ReadyConfiguration, SessionError> {
        let session = self
            .get_mut(machine_id)
            .ok_or(SessionError::NoSession(machine_id))?;
        let configuration = match session {
            MachineSession::ConfiguringDifficulty {
                player_id,
                difficulty,
                ..
            } => ReadyConfiguration {
                machine_id,
                player_id: *player_id,
                difficulty: difficulty.ok_or(SessionError::MissingDifficulty(machine_id))?,
            },
            _ => return Err(session.invalid("start game")),
        };
        *session = MachineSession::Starting(configuration);
        Ok(configuration)
    }

    pub fn rollback_start(&mut self, configuration: ReadyConfiguration) {
        if let Some(session @ MachineSession::Starting(_)) = self.get_mut(configuration.machine_id)
        {
            *session = MachineSession::ConfiguringDifficulty {
                machine_id: configuration.machine_id,
                player_id: configuration.player_id,
                difficulty: Some(configuration.difficulty),
            };
        }
    }

    pub fn start(&mut self, configuration: ReadyConfiguration, game_id: GameId, deadline: Instant) {
        // No RPC removes Starting sessions; the detached startup task owns this transition.
        let session = self
            .get_mut(configuration.machine_id)
            .expect("Starting session must exist");
        debug_assert!(matches!(session, MachineSession::Starting(_)));
        *session = MachineSession::Running {
            configuration,
            game_id,
            score: 0,
            deadline,
        };
    }

    pub fn remove_finished(&mut self, machine_id: MachineId, game_id: GameId) {
        for slot in [&mut self.0.0, &mut self.0.1] {
            if matches!(slot, Some(MachineSession::Finishing { configuration, game_id: id })
                if configuration.machine_id == machine_id && *id == game_id)
            {
                *slot = None;
                return;
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum MachineSession {
    ConfiguringGameMode {
        machine_id: MachineId,
        player_id: PlayerId,
    },
    ConfiguringDifficulty {
        machine_id: MachineId,
        player_id: PlayerId,
        difficulty: Option<Difficulty>,
    },
    Starting(ReadyConfiguration),
    Running {
        configuration: ReadyConfiguration,
        game_id: GameId,
        score: u32,
        deadline: Instant,
    },
    Finishing {
        configuration: ReadyConfiguration,
        game_id: GameId,
    },
}

impl MachineSession {
    pub fn machine_id(&self) -> MachineId {
        match self {
            Self::ConfiguringGameMode { machine_id, .. }
            | Self::ConfiguringDifficulty { machine_id, .. } => *machine_id,
            Self::Starting(c)
            | Self::Running {
                configuration: c, ..
            }
            | Self::Finishing {
                configuration: c, ..
            } => c.machine_id,
        }
    }

    fn player_id(&self) -> PlayerId {
        match self {
            Self::ConfiguringGameMode { player_id, .. }
            | Self::ConfiguringDifficulty { player_id, .. } => *player_id,
            Self::Starting(c)
            | Self::Running {
                configuration: c, ..
            }
            | Self::Finishing {
                configuration: c, ..
            } => c.player_id,
        }
    }

    fn is_configuring(&self) -> bool {
        matches!(
            self,
            Self::ConfiguringGameMode { .. } | Self::ConfiguringDifficulty { .. }
        )
    }

    fn invalid(&self, operation: &'static str) -> SessionError {
        let state = match self {
            Self::ConfiguringGameMode { .. } => "ConfiguringGameMode",
            Self::ConfiguringDifficulty { .. } => "ConfiguringDifficulty",
            Self::Starting(_) => "Starting",
            Self::Running { .. } => "Running",
            Self::Finishing { .. } => "Finishing",
        };
        SessionError::InvalidState {
            operation,
            machine_id: self.machine_id(),
            state,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sessions_insert_returns_err_when_called_on_same_machine_in_a_row() {
        let machine = MachineId::new(1);
        let mut sessions = Sessions::new();
        sessions.insert(machine, PlayerId::new(1)).unwrap();
        assert!(sessions.insert(machine, PlayerId::new(1)).is_ok());
        assert!(
            matches!(sessions.insert(machine, PlayerId::new(2)), Err(SessionError::AlreadyExists(id)) if id == machine)
        );
        sessions.set_single_mode(machine).unwrap();
        sessions
            .update_difficulty(machine, Difficulty::Normal)
            .unwrap();
        let reserved = sessions.reserve_start(machine).unwrap();
        assert!(sessions.reserve_start(machine).is_err());
        sessions.rollback_start(reserved);
        assert!(sessions.reserve_start(machine).is_ok());
    }

    #[test]
    fn sessions_update_difficulty_returns_err_when_called_in_configuring_mode() {
        let machine = MachineId::new(1);
        let mut sessions = Sessions::new();
        sessions.insert(machine, PlayerId::new(1)).unwrap();
        assert!(matches!(
            sessions.update_difficulty(machine, Difficulty::Normal),
            Err(SessionError::InvalidState { .. })
        ));
        sessions.set_single_mode(machine).unwrap();
        assert!(matches!(
            sessions.reserve_start(machine),
            Err(SessionError::MissingDifficulty(_))
        ));
        sessions
            .update_difficulty(machine, Difficulty::Hard)
            .unwrap();
        assert_eq!(
            sessions.reserve_start(machine).unwrap().difficulty,
            Difficulty::Hard
        );
    }
}
