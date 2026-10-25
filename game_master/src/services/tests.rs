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
    player_id: PlayerId,
}

impl DataSource for StubDataSource {
    async fn save_game(&self, _game: crate::GameRecord) -> Result<(), crate::SaveGameError> {
        Ok(())
    }

    async fn add_player(&self, _name: impl Into<String>) -> Result<PlayerId, AddPlayerError> {
        Ok(self.player_id)
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
    let service = GameMasterSessionServiceImpl::new(StubDataSource { player_id });
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
    let service = GameMasterSessionServiceImpl::new(StubDataSource { player_id });
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
    use struckout_proto::{add_target_request::TargetData, target::Compass};
    AddTargetRequest {
        machine_id,
        target_data: Some(TargetData::Compass(Compass {
            x: 0.0,
            y: 0.0,
            radius: 1.0,
        })),
    }
}

#[tokio::test]
async fn add_target_rejects_missing_data() {
    let service = GameMasterSessionServiceImpl::new(StubDataSource {
        player_id: PlayerId::new(1),
    });
    assert_eq!(
        service
            .add_target(Request::new(AddTargetRequest {
                machine_id: 1,
                target_data: None
            }))
            .await
            .expect_err("missing target data")
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
    assert_eq!(
        service
            .hit_target(Request::new(HitTargetRequest {
                machine_id: 1,
                target_id: Uuid::nil().to_string(),
            }))
            .await
            .expect_err("session is not running")
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
    assert_eq!(
        service
            .hit_target(Request::new(HitTargetRequest {
                machine_id: 1,
                target_id: Uuid::nil().to_string(),
            }))
            .await
            .expect_err("session is not running")
            .code(),
        tonic::Code::FailedPrecondition
    );
}

#[tokio::test(start_paused = true)]
async fn targets_can_be_added_and_removed_without_event_subscribers() {
    let service = GameMasterSessionServiceImpl::new(StubDataSource {
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

#[rstest]
#[case::single(GameMode::Single)]
#[case::multi(GameMode::Multi)]
#[tokio::test(start_paused = true)]
async fn hit_target_emits_the_updated_score(#[case] mode: GameMode) {
    let service = GameMasterSessionServiceImpl::new(StubDataSource {
        player_id: PlayerId::new(1),
    });
    let machine_count = if mode.is_multi() { 2 } else { 1 };
    for machine_id in 1..=machine_count {
        service
            .start_session(Request::new(StartSessionRequest {
                machine_id,
                player_id: machine_id,
            }))
            .await
            .expect("new session");
        service
            .set_game_mode(Request::new(SetGameModeRequest {
                machine_id,
                mode: mode.into(),
            }))
            .await
            .expect("select mode");
    }
    service
        .set_session_difficulty(Request::new(SetSessionDifficultyRequest {
            machine_id: 1,
            difficulty: Difficulty::Normal.into(),
        }))
        .await
        .expect("configure difficulty");
    for machine_id in 1..=machine_count {
        service
            .set_session_ready(Request::new(SetSessionReadyRequest { machine_id }))
            .await
            .expect("start game");
    }
    let target_id = service
        .add_target(Request::new(target_request(1)))
        .await
        .expect("add target")
        .into_inner()
        .target_id;
    let mut events = service.event_tx.subscribe();
    service
        .hit_target(Request::new(HitTargetRequest {
            machine_id: machine_count,
            target_id: target_id.clone(),
        }))
        .await
        .expect("hit target");
    let event = events.try_recv().expect("score event");
    assert_matches!(
        event.data,
        Ok(SuccessfulEvent::GameScoreChanged {
            total_score: 100_000
        })
    );
    if mode.is_multi() {
        assert_matches!(event.target, EventTarget::All);
        assert_eq!(
            service.sessions.read().score_multi().expect("shared score"),
            100_000
        );
    } else {
        assert_matches!(event.target, EventTarget::Single(id) if id == MachineId::new(1));
    }
    assert_eq!(
        service
            .sessions
            .read()
            .score(MachineId::new(1))
            .expect("running"),
        100_000
    );
    drop(events);
    service
        .hit_target(Request::new(HitTargetRequest {
            machine_id: 1,
            target_id,
        }))
        .await
        .expect("hit succeeds without subscribers");
    assert_eq!(
        service
            .sessions
            .read()
            .score(MachineId::new(1))
            .expect("running"),
        200_000
    );
}

#[rstest]
#[case::empty("", tonic::Code::InvalidArgument)]
#[case::invalid("invalid", tonic::Code::InvalidArgument)]
#[case::unknown("00000000-0000-0000-0000-000000000000", tonic::Code::NotFound)]
#[tokio::test(start_paused = true)]
async fn hit_target_rejects_invalid_ids(#[case] target_id: &str, #[case] code: tonic::Code) {
    let service = GameMasterSessionServiceImpl::new(StubDataSource {
        player_id: PlayerId::new(1),
    });
    start_game_single(
        &service,
        MachineId::new(1),
        Difficulty::Normal,
        PlayerId::new(1),
    )
    .await;
    let mut events = service.event_tx.subscribe();
    let error = service
        .hit_target(Request::new(HitTargetRequest {
            machine_id: 1,
            target_id: target_id.to_owned(),
        }))
        .await
        .expect_err("invalid target ID");
    assert_eq!(error.code(), code);
    assert_eq!(
        service
            .sessions
            .read()
            .score(MachineId::new(1))
            .expect("running"),
        0
    );
    assert_matches!(
        events.try_recv(),
        Err(broadcast::error::TryRecvError::Empty)
    );
}

#[rstest]
#[case::zero_radius(0.0, tonic::Code::FailedPrecondition)]
#[case::overflow(f32::MIN_POSITIVE, tonic::Code::OutOfRange)]
#[tokio::test(start_paused = true)]
async fn hit_target_rejects_invalid_scores(#[case] radius: f32, #[case] code: tonic::Code) {
    let service = GameMasterSessionServiceImpl::new(StubDataSource {
        player_id: PlayerId::new(1),
    });
    start_game_single(
        &service,
        MachineId::new(1),
        Difficulty::Normal,
        PlayerId::new(1),
    )
    .await;
    let mut req = target_request(1);
    let Some(struckout_proto::add_target_request::TargetData::Compass(compass)) =
        req.target_data.as_mut()
    else {
        panic!("compass target expected");
    };
    compass.radius = radius;
    let target_id = service
        .add_target(Request::new(req))
        .await
        .expect("add target")
        .into_inner()
        .target_id;
    let mut events = service.event_tx.subscribe();
    let error = service
        .hit_target(Request::new(HitTargetRequest {
            machine_id: 1,
            target_id,
        }))
        .await
        .expect_err("invalid score");
    assert_eq!(error.code(), code);
    assert_eq!(
        service
            .sessions
            .read()
            .score(MachineId::new(1))
            .expect("running"),
        0
    );
    assert_matches!(
        events.try_recv(),
        Err(broadcast::error::TryRecvError::Empty)
    );
}

#[derive(Clone, Default)]
struct RecordingDataSource {
    saved: Arc<parking_lot::Mutex<Vec<crate::GameRecord>>>,
    fail_save: bool,
}

impl DataSource for RecordingDataSource {
    async fn save_game(&self, game: crate::GameRecord) -> Result<(), crate::SaveGameError> {
        self.saved.lock().push(game);
        if self.fail_save {
            Err(sqlx::Error::PoolClosed.into())
        } else {
            Ok(())
        }
    }

    async fn add_player(&self, _: impl Into<String> + Send) -> Result<PlayerId, AddPlayerError> {
        unimplemented!()
    }

    async fn validate_player_name(
        &self,
        _: impl Into<String> + Send,
    ) -> Result<(), ValidatePlayerNameError> {
        unimplemented!()
    }

    async fn get_player(&self, _: impl Into<String> + Send) -> Result<Player, GetPlayerError> {
        unimplemented!()
    }
}

#[rstest]
#[case(GameMode::Single, false)]
#[case(GameMode::Multi, false)]
#[case(GameMode::Single, true)]
#[case(GameMode::Multi, true)]
#[tokio::test(start_paused = true)]
async fn results_are_retained_until_all_sessions_complete(
    #[case] mode: GameMode,
    #[case] fail_save: bool,
) {
    let ds = RecordingDataSource {
        fail_save,
        ..Default::default()
    };
    let service = GameMasterSessionServiceImpl::new(ds.clone());
    let mut events = service.event_tx.subscribe();
    let machines: &[u32] = if mode.is_multi() { &[1, 2] } else { &[1] };
    for &machine_id in machines {
        service
            .start_session(Request::new(StartSessionRequest {
                machine_id,
                player_id: machine_id + 10,
            }))
            .await
            .unwrap();
        service
            .set_game_mode(Request::new(SetGameModeRequest {
                machine_id,
                mode: mode.into(),
            }))
            .await
            .unwrap();
    }
    service
        .set_session_difficulty(Request::new(SetSessionDifficultyRequest {
            machine_id: 1,
            difficulty: Difficulty::Normal.into(),
        }))
        .await
        .unwrap();
    for &machine_id in machines {
        service
            .set_session_ready(Request::new(SetSessionReadyRequest { machine_id }))
            .await
            .unwrap();
    }
    assert!(
        ds.saved.lock().is_empty(),
        "starting a game must not write games to the DB"
    );
    assert_eq!(
        service
            .get_game_result(Request::new(GetGameResultRequest { machine_id: 1 }))
            .await
            .unwrap_err()
            .code(),
        tonic::Code::FailedPrecondition
    );
    assert_eq!(
        service
            .complete_session(Request::new(CompleteSessionRequest { machine_id: 1 }))
            .await
            .unwrap_err()
            .code(),
        tonic::Code::FailedPrecondition
    );
    let target_id = service
        .add_target(Request::new(target_request(1)))
        .await
        .unwrap()
        .into_inner()
        .target_id;
    service
        .hit_target(Request::new(HitTargetRequest {
            machine_id: 1,
            target_id: target_id.clone(),
        }))
        .await
        .unwrap();
    loop {
        if matches!(
            events.recv().await.unwrap().data,
            Ok(SuccessfulEvent::GameFinished)
        ) {
            break;
        }
    }
    assert!(
        ds.saved.lock().is_empty(),
        "GameFinished must not save the result"
    );
    for &machine_id in machines {
        assert_eq!(
            service
                .get_game_result(Request::new(GetGameResultRequest { machine_id }))
                .await
                .unwrap()
                .into_inner()
                .total_score,
            100_000
        );
    }
    assert_eq!(
        service
            .hit_target(Request::new(HitTargetRequest {
                machine_id: 1,
                target_id
            }))
            .await
            .unwrap_err()
            .code(),
        tonic::Code::FailedPrecondition
    );
    if mode.is_multi() {
        for _ in 0..2 {
            service
                .complete_session(Request::new(CompleteSessionRequest { machine_id: 1 }))
                .await
                .unwrap();
            assert!(ds.saved.lock().is_empty());
            let sessions = service.sessions.read();
            assert!(sessions.get(MachineId::new(1)).is_some());
            assert!(sessions.get(MachineId::new(2)).is_some());
            assert_eq!(
                sessions.game_result(MachineId::new(1)).unwrap().score,
                100_000
            );
        }
    }
    let last_machine = *machines.last().unwrap();
    let response = service
        .complete_session(Request::new(CompleteSessionRequest {
            machine_id: last_machine,
        }))
        .await;
    if fail_save {
        assert_eq!(response.unwrap_err().code(), tonic::Code::Internal);
    } else {
        response.unwrap();
    }
    for &machine_id in machines {
        assert!(
            service
                .sessions
                .read()
                .get(MachineId::new(machine_id))
                .is_none()
        );
        assert_eq!(
            service
                .get_game_result(Request::new(GetGameResultRequest { machine_id }))
                .await
                .unwrap_err()
                .code(),
            tonic::Code::FailedPrecondition
        );
        assert_eq!(
            service
                .complete_session(Request::new(CompleteSessionRequest { machine_id }))
                .await
                .unwrap_err()
                .code(),
            tonic::Code::FailedPrecondition
        );
    }
    let saved = ds.saved.lock();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].score, 100_000);
    assert_eq!(saved[0].difficulty, Difficulty::Normal);
    assert_eq!(
        saved[0].players,
        if mode.is_multi() {
            crate::GamePlayers::Multi {
                player_ids: [PlayerId::new(11), PlayerId::new(12)],
            }
        } else {
            crate::GamePlayers::Single {
                machine_id: MachineId::new(1),
                player_id: PlayerId::new(11),
            }
        }
    );
}

#[tokio::test]
async fn complete_session_rejects_configuring_and_missing_sessions() {
    let service = GameMasterSessionServiceImpl::new(RecordingDataSource::default());
    for start in [false, true] {
        if start {
            service
                .start_session(Request::new(StartSessionRequest {
                    machine_id: 1,
                    player_id: 11,
                }))
                .await
                .unwrap();
        }
        assert_eq!(
            service
                .complete_session(Request::new(CompleteSessionRequest { machine_id: 1 }))
                .await
                .unwrap_err()
                .code(),
            tonic::Code::FailedPrecondition
        );
    }
    assert!(service.sessions.read().get(MachineId::new(1)).is_some());
}
