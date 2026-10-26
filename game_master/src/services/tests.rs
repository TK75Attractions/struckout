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

/// Configures participants without starting a timer, so each RPC test controls its state.
fn configured_recorded_service(
    mode: GameMode,
    data_source: RecordingDataSource,
) -> GameMasterSessionServiceImpl<RecordingDataSource> {
    let service = GameMasterSessionServiceImpl::new(data_source);
    let mut sessions = service.sessions.write();
    for machine_id in 1..=if mode.is_multi() { 2 } else { 1 } {
        let machine_id = MachineId::new(machine_id);
        sessions
            .insert(machine_id, PlayerId::new(machine_id.into_inner() + 10))
            .expect("new session");
        sessions
            .set_game_mode(machine_id, mode)
            .expect("select mode");
    }
    sessions
        .update_difficulty(MachineId::new(1), Difficulty::Normal)
        .expect("configure difficulty");
    drop(sessions);
    service
}

/// Starts session state directly because most tests exercise result and completion RPCs.
fn running_recorded_service(
    mode: GameMode,
    data_source: RecordingDataSource,
) -> GameMasterSessionServiceImpl<RecordingDataSource> {
    let service = configured_recorded_service(mode, data_source);
    let mut sessions = service.sessions.write();
    for machine_id in 1..=if mode.is_multi() { 2 } else { 1 } {
        sessions
            .set_ready(MachineId::new(machine_id))
            .expect("start game");
    }
    let (state,) = sessions
        .get(MachineId::new(1))
        .expect("session exists")
        .as_running()
        .expect("running game");
    state.shared.write().score = 123;
    drop(sessions);
    service
}

fn finished_recorded_service(
    mode: GameMode,
    data_source: RecordingDataSource,
) -> GameMasterSessionServiceImpl<RecordingDataSource> {
    let service = running_recorded_service(mode, data_source);
    service
        .sessions
        .write()
        .finish_game(MachineId::new(1))
        .expect("finish running game");
    service
}

#[rstest]
#[case::single(GameMode::Single)]
#[case::multi(GameMode::Multi)]
#[tokio::test(start_paused = true)]
async fn set_session_ready_does_not_save_game(#[case] mode: GameMode) {
    let ds = RecordingDataSource::default();
    let service = configured_recorded_service(mode, ds.clone());

    for machine_id in 1..=if mode.is_multi() { 2 } else { 1 } {
        service
            .set_session_ready(Request::new(SetSessionReadyRequest { machine_id }))
            .await
            .expect("start game");
    }

    assert!(ds.saved.lock().is_empty());
}

#[rstest]
#[case::single(GameMode::Single, EventTarget::Single(MachineId::new(1)))]
#[case::multi(GameMode::Multi, EventTarget::All)]
#[tokio::test]
async fn game_completion_retains_results_without_saving(
    #[case] mode: GameMode,
    #[case] target: EventTarget,
) {
    let ds = RecordingDataSource::default();
    let service = running_recorded_service(mode, ds.clone());
    let (tx, rx) = mpsc::channel(1);
    tx.send(()).await.expect("notify game completion");

    service.clone().handle_game_completion(target, rx).await;

    assert!(ds.saved.lock().is_empty());
    let sessions = service.sessions.read();
    for machine_id in 1..=if mode.is_multi() { 2 } else { 1 } {
        assert_eq!(
            sessions
                .game_result(MachineId::new(machine_id))
                .expect("result retained")
                .score,
            123
        );
    }
}

#[rstest]
#[case::single(GameMode::Single)]
#[case::multi(GameMode::Multi)]
#[tokio::test]
async fn complete_session_saves_result_and_discards_sessions(#[case] mode: GameMode) {
    let ds = RecordingDataSource::default();
    let service = finished_recorded_service(mode, ds.clone());
    let expected = service
        .sessions
        .read()
        .game_result(MachineId::new(1))
        .expect("finished result")
        .clone();
    let machine_count = if mode.is_multi() { 2 } else { 1 };

    for machine_id in 1..=machine_count {
        service
            .complete_session(Request::new(CompleteSessionRequest { machine_id }))
            .await
            .expect("complete finished session");
    }

    assert_eq!(*ds.saved.lock(), [expected]);
    for machine_id in 1..=machine_count {
        assert!(
            service
                .sessions
                .read()
                .get(MachineId::new(machine_id))
                .is_none()
        );
    }
}

#[rstest]
#[case::first_machine(1)]
#[case::second_machine(2)]
#[tokio::test]
async fn multi_play_completion_waits_for_the_other_machine(#[case] machine_id: u32) {
    let ds = RecordingDataSource::default();
    let service = finished_recorded_service(GameMode::Multi, ds.clone());

    service
        .complete_session(Request::new(CompleteSessionRequest { machine_id }))
        .await
        .expect("acknowledge first completion");

    assert!(ds.saved.lock().is_empty());
    let sessions = service.sessions.read();
    for machine_id in [1, 2] {
        assert!(sessions.get(MachineId::new(machine_id)).is_some());
    }
}

#[tokio::test]
async fn repeated_multi_play_completion_keeps_waiting_for_the_other_machine() {
    let ds = RecordingDataSource::default();
    let service = finished_recorded_service(GameMode::Multi, ds.clone());
    service
        .complete_session(Request::new(CompleteSessionRequest { machine_id: 1 }))
        .await
        .expect("acknowledge first completion");

    service
        .complete_session(Request::new(CompleteSessionRequest { machine_id: 1 }))
        .await
        .expect("repeated completion succeeds");

    assert!(ds.saved.lock().is_empty());
    let sessions = service.sessions.read();
    for machine_id in [1, 2] {
        assert!(sessions.get(MachineId::new(machine_id)).is_some());
    }
}

#[rstest]
#[case::single(GameMode::Single)]
#[case::multi(GameMode::Multi)]
#[tokio::test]
async fn complete_session_discards_sessions_when_save_fails(#[case] mode: GameMode) {
    let service = finished_recorded_service(
        mode,
        RecordingDataSource {
            fail_save: true,
            ..Default::default()
        },
    );
    let last_machine = if mode.is_multi() {
        service
            .complete_session(Request::new(CompleteSessionRequest { machine_id: 1 }))
            .await
            .expect("wait for the other machine");
        2
    } else {
        1
    };

    let error = service
        .complete_session(Request::new(CompleteSessionRequest {
            machine_id: last_machine,
        }))
        .await
        .expect_err("database save fails");

    assert_eq!(error.code(), tonic::Code::Internal);
    for machine_id in 1..=last_machine {
        assert!(
            service
                .sessions
                .read()
                .get(MachineId::new(machine_id))
                .is_none()
        );
    }
}

#[rstest]
#[case::single(GameMode::Single)]
#[case::multi(GameMode::Multi)]
#[tokio::test]
async fn get_game_result_returns_the_current_session_score(#[case] mode: GameMode) {
    let service = finished_recorded_service(mode, RecordingDataSource::default());

    for machine_id in 1..=if mode.is_multi() { 2 } else { 1 } {
        let result = service
            .get_game_result(Request::new(GetGameResultRequest { machine_id }))
            .await
            .expect("finished result")
            .into_inner();

        assert_eq!(result.total_score, 123);
    }
}

#[tokio::test]
async fn get_game_result_remains_available_while_waiting_for_session_completion() {
    let service = finished_recorded_service(GameMode::Multi, RecordingDataSource::default());
    service
        .complete_session(Request::new(CompleteSessionRequest { machine_id: 1 }))
        .await
        .expect("wait for the other machine");

    let result = service
        .get_game_result(Request::new(GetGameResultRequest { machine_id: 1 }))
        .await
        .expect("result retained while waiting")
        .into_inner();

    assert_eq!(result.total_score, 123);
}

#[rstest]
#[case::missing(false)]
#[case::running(true)]
#[tokio::test]
async fn get_game_result_rejects_sessions_without_a_finished_result(#[case] running: bool) {
    let ds = RecordingDataSource::default();
    let service = if running {
        running_recorded_service(GameMode::Single, ds)
    } else {
        GameMasterSessionServiceImpl::new(ds)
    };

    let error = service
        .get_game_result(Request::new(GetGameResultRequest { machine_id: 1 }))
        .await
        .expect_err("no finished result");

    assert_eq!(error.code(), tonic::Code::FailedPrecondition);
}

#[tokio::test]
async fn complete_session_rejects_running_sessions() {
    let service = running_recorded_service(GameMode::Single, RecordingDataSource::default());

    let error = service
        .complete_session(Request::new(CompleteSessionRequest { machine_id: 1 }))
        .await
        .expect_err("game is still running");

    assert_eq!(error.code(), tonic::Code::FailedPrecondition);
}

#[tokio::test]
async fn hit_target_rejects_finished_sessions() {
    let service = finished_recorded_service(GameMode::Single, RecordingDataSource::default());

    let error = service
        .hit_target(Request::new(HitTargetRequest {
            machine_id: 1,
            target_id: Uuid::nil().to_string(),
        }))
        .await
        .expect_err("game has finished");

    assert_eq!(error.code(), tonic::Code::FailedPrecondition);
}

#[rstest]
#[case::missing(false)]
#[case::configuring(true)]
#[tokio::test]
async fn complete_session_rejects_configuring_and_missing_sessions(#[case] configuring: bool) {
    let service = GameMasterSessionServiceImpl::new(RecordingDataSource::default());
    if configuring {
        service
            .start_session(Request::new(StartSessionRequest {
                machine_id: 1,
                player_id: 11,
            }))
            .await
            .expect("new session");
    }

    let error = service
        .complete_session(Request::new(CompleteSessionRequest { machine_id: 1 }))
        .await
        .expect_err("session has no finished game");

    assert_eq!(error.code(), tonic::Code::FailedPrecondition);
}
