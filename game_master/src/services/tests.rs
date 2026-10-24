//! Tests of [`GameMasterSessionServiceImpl`].
use std::assert_matches;

use rstest::rstest;
use struckout_proto::event::EventData;

use crate::{
    AddPlayerError, GetPlayerError, PlayerId, ValidatePlayerNameError,
    data::Player,
    session::{ConfiguringDifficultyState, ConfiguringGameModeState},
};

use super::*;

#[derive(Clone)]
struct StubDataSource {
    game_id: GameId,
    player_id: PlayerId,
}

impl DataSource for StubDataSource {
    async fn insert_game_single(
        &self,
        _machine_id: MachineId,
        _player_id: PlayerId,
        _started_at: UtcDateTime,
        _difficulty: Difficulty,
    ) -> Result<GameId, sqlx::Error> {
        Ok(self.game_id)
    }

    async fn insert_game_multi(
        &self,
        _player_ids: [PlayerId; 2],
        _started_at: UtcDateTime,
        _difficulty: Difficulty,
    ) -> Result<GameId, sqlx::Error> {
        unimplemented!()
    }

    async fn complete_game(&self, _game_id: GameId, _score: u32) -> Result<(), sqlx::Error> {
        Ok(())
    }

    async fn add_player(&self, _name: impl Into<String>) -> Result<PlayerId, AddPlayerError> {
        Ok(self.player_id)
    }

    async fn get_game_result(
        &self,
        _game_id: GameId,
    ) -> Result<crate::data::GameRecord, crate::GetGameResultError> {
        unimplemented!()
    }

    async fn validate_player_name(
        &self,
        _name: impl Into<String>,
    ) -> Result<(), ValidatePlayerNameError> {
        unimplemented!()
    }

    async fn get_player(&self, _name: impl Into<String> + Send) -> Result<Player, GetPlayerError> {
        unimplemented!()
    }
}

/// Waits for event matching `ev_pat`. Returns when a event from stream matches pattern, or panics when stream ends.
#[allow(unused)] // temporary allow
macro_rules! wait_event {
    ($stream_var:ident, $ev_pat:pat) => {
        loop {
            use tokio_stream::StreamExt;
            let Some(ev) = $stream_var.next().await else {
                panic!("event stream ended");
            };
            match ev {
                Ok(ListenEventsResponse {
                    event:
                        Some(::struckout_proto::Event {
                            event_data: Some(ret @ $ev_pat),
                            ..
                        }),
                }) => break Some(ret),
                _ => (),
            }
        }
    };
}

/// Asserts that next event matches pattern, returning the event if matches.
macro_rules! assert_event {
    ($stream_var:ident, $ev_pat:pat) => {{
        use tokio_stream::StreamExt;

        let Some(ev) = $stream_var.next().await else {
            panic!("event stream ended");
        };
        match ev {
            Ok(ListenEventsResponse {
                event:
                    Some(::struckout_proto::Event {
                        event_data: Some(ret @ $ev_pat),
                        ..
                    }),
            }) => ret,
            el => panic!(
                "received unexpected event: expected {}, actually {:?}",
                stringify!($ev_pat),
                el
            ),
        }
    }};
}

/// Starts single-play game.
async fn start_game_single<DS>(
    service: &GameMasterSessionServiceImpl<DS>,
    machine_id: MachineId,
    difficulty: Difficulty,
    player_id: PlayerId,
) where
    DS: DataSource,
{
    service
        .start_session(Request::new(StartSessionRequest {
            machine_id: machine_id.into(),
            player_id: player_id.into(),
        }))
        .await
        .expect("should succeed");
    service
        .set_game_mode(Request::new(SetGameModeRequest {
            mode: GameMode::Single.into(),
            machine_id: machine_id.into(),
        }))
        .await
        .expect("should succeed");
    service
        .set_session_difficulty(Request::new(SetSessionDifficultyRequest {
            difficulty: difficulty.into(),
            machine_id: machine_id.into(),
        }))
        .await
        .expect("should succeed");
    service
        .set_session_ready(Request::new(SetSessionReadyRequest {
            machine_id: machine_id.into(),
        }))
        .await
        .expect("should succeed");
}

#[rstest]
#[case::single(GameMode::Single)]
#[case::multi(GameMode::Multi)]
#[tokio::test]
async fn set_game_mode_advances_to_difficulty_configuration(#[case] mode: GameMode) {
    let machine_id = MachineId::new(1);
    let player_id = PlayerId::new(2);
    let service = GameMasterSessionServiceImpl::new(StubDataSource {
        game_id: GameId::new(3),
        player_id,
    });
    service
        .start_session(Request::new(StartSessionRequest {
            machine_id: machine_id.into(),
            player_id: player_id.into(),
        }))
        .await
        .expect("should succeed");

    service
        .set_game_mode(Request::new(SetGameModeRequest {
            machine_id: machine_id.into(),
            mode: mode.into(),
        }))
        .await
        .expect("should succeed without event subscribers");

    assert_matches!(
        service.sessions.read().get(machine_id),
        Some(crate::session::MachineSession::ConfiguringDifficulty(ConfiguringDifficultyState {
            mode: actual_mode, difficulty: None, ..
        })) if *actual_mode == mode
    );
    let error = service
        .set_game_mode(Request::new(SetGameModeRequest {
            machine_id: machine_id.into(),
            mode: mode.into(),
        }))
        .await
        .expect_err("mode has already been selected");
    assert_eq!(error.code(), tonic::Code::FailedPrecondition);
}

#[rstest]
#[case(GameMode::Unspecified.into())]
#[case(-1)]
#[case(i32::MAX)]
#[tokio::test]
async fn set_game_mode_rejects_invalid_modes(#[case] mode: i32) {
    let machine_id = MachineId::new(1);
    let player_id = PlayerId::new(2);
    let service = GameMasterSessionServiceImpl::new(StubDataSource {
        game_id: GameId::new(3),
        player_id,
    });
    service
        .start_session(Request::new(StartSessionRequest {
            machine_id: machine_id.into(),
            player_id: player_id.into(),
        }))
        .await
        .expect("should succeed");

    let error = service
        .set_game_mode(Request::new(SetGameModeRequest {
            machine_id: machine_id.into(),
            mode,
        }))
        .await
        .expect_err("invalid mode must be rejected");
    assert_eq!(error.code(), tonic::Code::InvalidArgument);
    assert_matches!(
        service.sessions.read().get(machine_id),
        Some(crate::session::MachineSession::ConfiguringGameMode(
            ConfiguringGameModeState { mode: None, .. }
        ))
    );
}

#[tokio::test]
async fn set_game_mode_requires_an_existing_session() {
    let service = GameMasterSessionServiceImpl::new(StubDataSource {
        game_id: GameId::new(3),
        player_id: PlayerId::new(2),
    });
    let error = service
        .set_game_mode(Request::new(SetGameModeRequest {
            machine_id: 1,
            mode: GameMode::Single.into(),
        }))
        .await
        .expect_err("session has not been started");
    assert_eq!(error.code(), tonic::Code::FailedPrecondition);
}

#[tokio::test]
async fn set_session_ready_triggers_game_started_event() {
    let ds = StubDataSource {
        game_id: GameId::new(14),
        player_id: PlayerId::new(334),
    };
    let machine_id = MachineId::new(1);
    let difficulty = Difficulty::Normal;
    let service = GameMasterSessionServiceImpl::new(ds.clone());

    let stream = service
        .listen_events(Request::new(ListenEventsRequest {
            machine_id: machine_id.into(),
        }))
        .await
        .expect("should succeed");
    let mut stream = stream.into_inner();
    service
        .start_session(Request::new(StartSessionRequest {
            machine_id: machine_id.into(),
            player_id: ds.player_id.into(),
        }))
        .await
        .expect("should succeed");
    service
        .set_game_mode(Request::new(SetGameModeRequest {
            mode: GameMode::Single.into(),
            machine_id: machine_id.into(),
        }))
        .await
        .expect("should succeed");
    service
        .set_session_difficulty(Request::new(SetSessionDifficultyRequest {
            difficulty: difficulty.into(),
            machine_id: machine_id.into(),
        }))
        .await
        .expect("should succeed");
    assert_event!(stream, EventData::SessionDifficultyChanged(_));

    service
        .set_session_ready(Request::new(SetSessionReadyRequest {
            machine_id: machine_id.into(),
        }))
        .await
        .expect("should succeed");

    assert_event!(
        stream,
        EventData::GameStarted(struckout_proto::event::GameStarted { .. })
    );
}

#[tokio::test(start_paused = true)]
async fn event_stream_notifies_in_correct_order() {
    let ds = StubDataSource {
        game_id: GameId::new(20),
        player_id: PlayerId::new(13),
    };
    let machine_id = MachineId::new(2);
    let difficulty = Difficulty::Normal;
    let service = GameMasterSessionServiceImpl::new(ds.clone());

    let stream = service
        .listen_events(Request::new(ListenEventsRequest {
            machine_id: machine_id.into_inner(),
        }))
        .await
        .expect("should succeed");
    let mut stream = stream.into_inner();

    start_game_single(&service, machine_id, difficulty, PlayerId::new(1)).await;

    // Assert: Started
    assert_event!(stream, EventData::SessionDifficultyChanged(_));
    assert_event!(stream, EventData::GameStarted(_));

    // Assert: TimeLimitNotify
    let finished = loop {
        let resp = stream.next().await.unwrap();
        let resp = resp.expect("should not have error");
        let ev = resp.event.unwrap();
        let ev = ev.event_data.unwrap();
        let EventData::GameTimeLimitNotify(notify) = ev else {
            break ev;
        };
        let _rem = notify.remaining.unwrap();
    };

    // Assert: Finished
    assert_matches!(finished, EventData::GameFinished(_));
}

fn target_request(machine_id: u32) -> AddTargetRequest {
    use struckout_proto::{
        Target,
        target::{Compass, Owner, OwnerAll, TargetData},
    };
    AddTargetRequest {
        target: Some(Target {
            machine_id,
            target_data: Some(TargetData::Compass(Compass {
                x: 0.0,
                y: 0.0,
                radius: 1.0,
            })),
            owner: Some(Owner::All(OwnerAll {})),
        }),
    }
}

#[rstest]
#[case::missing_target(AddTargetRequest { target: None })]
#[case::missing_data({
    let mut req = target_request(1);
    req.target.as_mut().expect("target exists").target_data = None;
    req
})]
#[case::missing_owner({
    let mut req = target_request(1);
    req.target.as_mut().expect("target exists").owner = None;
    req
})]
#[tokio::test]
async fn add_target_rejects_missing_fields(#[case] req: AddTargetRequest) {
    let service = GameMasterSessionServiceImpl::new(StubDataSource {
        game_id: GameId::new(1),
        player_id: PlayerId::new(1),
    });
    assert_eq!(
        service
            .add_target(Request::new(req))
            .await
            .expect_err("invalid target")
            .code(),
        tonic::Code::InvalidArgument
    );
}

#[rstest]
#[case::empty("")]
#[case::invalid("invalid")]
#[tokio::test]
async fn remove_target_rejects_invalid_ids(#[case] target_id: &str) {
    let service = GameMasterSessionServiceImpl::new(StubDataSource {
        game_id: GameId::new(1),
        player_id: PlayerId::new(1),
    });
    assert_eq!(
        service
            .remove_target(Request::new(RemoveTargetRequest {
                machine_id: 1,
                target_id: target_id.to_owned(),
            }))
            .await
            .expect_err("invalid UUID")
            .code(),
        tonic::Code::InvalidArgument
    );
}

#[tokio::test]
async fn target_operations_require_a_running_session() {
    let service = GameMasterSessionServiceImpl::new(StubDataSource {
        game_id: GameId::new(1),
        player_id: PlayerId::new(1),
    });
    assert_eq!(
        service
            .add_target(Request::new(target_request(1)))
            .await
            .expect_err("missing session")
            .code(),
        tonic::Code::FailedPrecondition
    );
    assert_eq!(
        service
            .remove_target(Request::new(RemoveTargetRequest {
                machine_id: 1,
                target_id: Uuid::nil().to_string()
            }))
            .await
            .expect_err("missing session")
            .code(),
        tonic::Code::FailedPrecondition
    );
    service
        .start_session(Request::new(StartSessionRequest {
            machine_id: 1,
            player_id: 1,
        }))
        .await
        .expect("new session");
    assert_eq!(
        service
            .add_target(Request::new(target_request(1)))
            .await
            .expect_err("not running")
            .code(),
        tonic::Code::FailedPrecondition
    );
    assert_eq!(
        service
            .remove_target(Request::new(RemoveTargetRequest {
                machine_id: 1,
                target_id: Uuid::nil().to_string()
            }))
            .await
            .expect_err("not running")
            .code(),
        tonic::Code::FailedPrecondition
    );
}

#[tokio::test(start_paused = true)]
async fn targets_can_be_added_and_removed_without_event_subscribers() {
    let service = GameMasterSessionServiceImpl::new(StubDataSource {
        game_id: GameId::new(1),
        player_id: PlayerId::new(1),
    });
    start_game_single(
        &service,
        MachineId::new(1),
        Difficulty::Normal,
        PlayerId::new(1),
    )
    .await;
    let target_id = service
        .add_target(Request::new(target_request(1)))
        .await
        .expect("running session")
        .into_inner()
        .target_id;
    Uuid::parse_str(&target_id).expect("UUID response");
    service
        .remove_target(Request::new(RemoveTargetRequest {
            machine_id: 1,
            target_id: target_id.clone(),
        }))
        .await
        .expect("registered target");
    assert_eq!(
        service
            .remove_target(Request::new(RemoveTargetRequest {
                machine_id: 1,
                target_id
            }))
            .await
            .expect_err("already removed")
            .code(),
        tonic::Code::NotFound
    );
}
