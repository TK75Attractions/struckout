//! Tests of [`GameMasterSessionServiceImpl`].
use std::assert_matches;

use rstest::rstest;
use struckout_proto::{event::EventData, types::GameId};

use super::*;
use crate::{
    AddPlayerError, GetPlayerError, PlayerId, ValidatePlayerNameError,
    data::Player,
    session::{ConfiguringDifficultyState, ConfiguringGameModeState},
};

#[derive(Clone)]
struct StubDataSource {
    player_id: PlayerId,
}

impl DataSource for StubDataSource {
    async fn save_game(&self, _game: crate::GameRecord) -> Result<GameId, crate::SaveGameError> {
        Ok(GameId::new(1))
    }

    async fn get_game_result(&self, game_id: GameId) -> Result<u32, crate::GetGameResultError> {
        Err(crate::GetGameResultError::NotFound(game_id))
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

    assert_event!(stream, EventData::TargetAdded(_));

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

#[tokio::test]
async fn target_events_are_sent_only_to_the_owning_machine() {
    let service = running_recorded_service(GameMode::Multi, RecordingDataSource::default());
    let mut first = service
        .listen_events(Request::new(ListenEventsRequest { machine_id: 1 }))
        .await
        .expect("first stream")
        .into_inner();
    let mut second = service
        .listen_events(Request::new(ListenEventsRequest { machine_id: 2 }))
        .await
        .expect("second stream")
        .into_inner();

    let first_id = service
        .add_target(Request::new(target_request(1)))
        .await
        .expect("first target")
        .into_inner()
        .target_id;
    let second_id = service
        .add_target(Request::new(target_request(2)))
        .await
        .expect("second target")
        .into_inner()
        .target_id;

    let EventData::TargetAdded(first_added) = assert_event!(first, EventData::TargetAdded(_))
    else {
        unreachable!()
    };
    let EventData::TargetAdded(second_added) = assert_event!(second, EventData::TargetAdded(_))
    else {
        unreachable!()
    };
    let first_target = first_added.target.expect("target payload");
    let second_target = second_added.target.expect("target payload");
    assert_eq!(first_target.target_id, first_id);
    assert_eq!(second_target.target_id, second_id);
    assert_matches!(first_target.owner, Some(Owner::Single(owner)) if owner.machine_id == 1);
    assert_matches!(second_target.owner, Some(Owner::Single(owner)) if owner.machine_id == 2);
    assert_eq!(
        first_target.target_data,
        target_request(1).target_data.map(|data| match data {
            struckout_proto::add_target_request::TargetData::Compass(compass) =>
                struckout_proto::target::TargetData::Compass(compass),
        })
    );

    service
        .remove_target(Request::new(RemoveTargetRequest {
            machine_id: 2,
            target_id: first_id.clone(),
        }))
        .await
        .expect("shared target can be removed from another machine");
    service
        .remove_target(Request::new(RemoveTargetRequest {
            machine_id: 1,
            target_id: second_id.clone(),
        }))
        .await
        .expect("shared target can be removed from another machine");

    let EventData::TargetRemoved(first_removed) = assert_event!(first, EventData::TargetRemoved(_))
    else {
        unreachable!()
    };
    let EventData::TargetRemoved(second_removed) =
        assert_event!(second, EventData::TargetRemoved(_))
    else {
        unreachable!()
    };
    assert_eq!(first_removed.target_id, first_id);
    assert_eq!(second_removed.target_id, second_id);
    assert!(
        tokio::time::timeout(Duration::from_millis(10), first.next())
            .await
            .is_err()
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(10), second.next())
            .await
            .is_err()
    );
}

#[rstest]
#[case::single(GameMode::Single, EventTarget::Single(MachineId::new(1)))]
#[case::multi(GameMode::Multi, EventTarget::All)]
#[tokio::test]
async fn completion_does_not_emit_target_removals(
    #[case] mode: GameMode,
    #[case] target: EventTarget,
) {
    let service = running_recorded_service(mode, RecordingDataSource::default());
    let mut first = service
        .listen_events(Request::new(ListenEventsRequest { machine_id: 1 }))
        .await
        .expect("first stream")
        .into_inner();
    let mut second = service
        .listen_events(Request::new(ListenEventsRequest { machine_id: 2 }))
        .await
        .expect("second stream")
        .into_inner();

    service
        .add_target(Request::new(target_request(1)))
        .await
        .expect("first target");
    assert_event!(first, EventData::TargetAdded(_));
    if mode.is_multi() {
        service
            .add_target(Request::new(target_request(2)))
            .await
            .expect("second target");
        assert_event!(second, EventData::TargetAdded(_));
    }

    complete_game(&service, target).await;

    assert_event!(first, EventData::GameFinished(_));
    if mode.is_multi() {
        assert_event!(second, EventData::GameFinished(_));
    }
    assert!(
        tokio::time::timeout(Duration::from_millis(10), first.next())
            .await
            .is_err()
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(10), second.next())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn completion_does_not_emit_shared_target_removals() {
    let service = running_recorded_service(GameMode::Multi, RecordingDataSource::default());
    let mut first = service
        .listen_events(Request::new(ListenEventsRequest { machine_id: 1 }))
        .await
        .expect("first stream")
        .into_inner();
    let mut second = service
        .listen_events(Request::new(ListenEventsRequest { machine_id: 2 }))
        .await
        .expect("second stream")
        .into_inner();
    service
        .sessions
        .write()
        .add_target(
            struckout_proto::target::TargetData::Compass(struckout_proto::target::Compass {
                x: 0.0,
                y: 0.0,
                radius: 1.0,
            }),
            Owner::All(Default::default()),
        )
        .expect("shared target");

    complete_game(&service, EventTarget::All).await;

    for stream in [&mut first, &mut second] {
        assert_event!(stream, EventData::GameFinished(_));
        assert!(
            tokio::time::timeout(Duration::from_millis(10), stream.next())
                .await
                .is_err()
        );
    }
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
    fail_get_result: bool,
    save_started: Arc<tokio::sync::Notify>,
    save_gate: Option<Arc<tokio::sync::Semaphore>>,
}

impl DataSource for RecordingDataSource {
    async fn save_game(&self, game: crate::GameRecord) -> Result<GameId, crate::SaveGameError> {
        self.saved.lock().push(game);
        self.save_started.notify_one();
        if let Some(gate) = &self.save_gate {
            gate.acquire()
                .await
                .expect("save gate remains open")
                .forget();
        }
        if self.fail_save {
            Err(sqlx::Error::PoolClosed.into())
        } else {
            Ok(GameId::new(42))
        }
    }

    async fn get_game_result(&self, game_id: GameId) -> Result<u32, crate::GetGameResultError> {
        if self.fail_get_result {
            return Err(sqlx::Error::PoolClosed.into());
        }
        if game_id == GameId::new(42) {
            self.saved
                .lock()
                .first()
                .map(|game| game.score)
                .ok_or(crate::GetGameResultError::NotFound(game_id))
        } else {
            Err(crate::GetGameResultError::NotFound(game_id))
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

async fn complete_game(
    service: &GameMasterSessionServiceImpl<RecordingDataSource>,
    target: EventTarget,
) {
    let (tx, rx) = mpsc::channel(1);
    tx.send(()).await.expect("notify game completion");
    service.clone().handle_game_completion(target, rx).await;
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
async fn game_completion_saves_the_final_score(
    #[case] mode: GameMode,
    #[case] target: EventTarget,
) {
    let ds = RecordingDataSource::default();
    let service = running_recorded_service(mode, ds.clone());

    complete_game(&service, target).await;

    let saved = ds.saved.lock();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].score, 123);
}

#[rstest]
#[case::single(GameMode::Single, EventTarget::Single(MachineId::new(1)))]
#[case::multi(GameMode::Multi, EventTarget::All)]
#[tokio::test]
async fn game_finished_contains_the_saved_game_id(
    #[case] mode: GameMode,
    #[case] target: EventTarget,
) {
    let service = running_recorded_service(mode, RecordingDataSource::default());
    let mut stream = service
        .listen_events(Request::new(ListenEventsRequest { machine_id: 1 }))
        .await
        .expect("listen for completion")
        .into_inner();

    complete_game(&service, target).await;

    assert_event!(
        stream,
        EventData::GameFinished(struckout_proto::event::GameFinished { game_id: 42 })
    );
}

#[rstest]
#[case::single(GameMode::Single, EventTarget::Single(MachineId::new(1)))]
#[case::multi(GameMode::Multi, EventTarget::All)]
#[tokio::test]
async fn game_completion_discards_sessions_after_saving(
    #[case] mode: GameMode,
    #[case] target: EventTarget,
) {
    let gate = Arc::new(tokio::sync::Semaphore::new(0));
    let ds = RecordingDataSource {
        save_gate: Some(gate.clone()),
        ..Default::default()
    };
    let service = running_recorded_service(mode, ds.clone());
    let task = tokio::spawn({
        let service = service.clone();
        async move { complete_game(&service, target).await }
    });
    ds.save_started.notified().await;
    for machine_id in 1..=if mode.is_multi() { 2 } else { 1 } {
        assert!(
            service
                .sessions
                .read()
                .get(MachineId::new(machine_id))
                .is_some()
        );
    }

    gate.add_permits(1);
    task.await.expect("game completion task succeeds");

    for machine_id in 1..=if mode.is_multi() { 2 } else { 1 } {
        assert!(
            service
                .sessions
                .read()
                .get(MachineId::new(machine_id))
                .is_none()
        );
    }
}

#[tokio::test]
async fn game_finished_waits_for_the_database_save() {
    let gate = Arc::new(tokio::sync::Semaphore::new(0));
    let ds = RecordingDataSource {
        save_gate: Some(gate.clone()),
        ..Default::default()
    };
    let service = running_recorded_service(GameMode::Single, ds.clone());
    let mut events = service.event_tx.subscribe();
    let task = tokio::spawn({
        let service = service.clone();
        async move { complete_game(&service, EventTarget::Single(MachineId::new(1))).await }
    });
    ds.save_started.notified().await;

    assert_matches!(
        events.try_recv(),
        Err(broadcast::error::TryRecvError::Empty)
    );
    gate.add_permits(1);
    task.await.expect("game completion task succeeds");

    assert_matches!(
        events.try_recv().expect("completion event").data,
        Ok(SuccessfulEvent::GameFinished { .. })
    );
}

#[rstest]
#[case::single(GameMode::Single, EventTarget::Single(MachineId::new(1)))]
#[case::multi(GameMode::Multi, EventTarget::All)]
#[tokio::test]
async fn game_completion_discards_sessions_when_save_fails(
    #[case] mode: GameMode,
    #[case] target: EventTarget,
) {
    let service = running_recorded_service(
        mode,
        RecordingDataSource {
            fail_save: true,
            ..Default::default()
        },
    );

    complete_game(&service, target).await;

    for machine_id in 1..=if mode.is_multi() { 2 } else { 1 } {
        assert!(
            service
                .sessions
                .read()
                .get(MachineId::new(machine_id))
                .is_none()
        );
    }
}

#[tokio::test]
async fn game_completion_notifies_a_save_error() {
    let service = running_recorded_service(
        GameMode::Single,
        RecordingDataSource {
            fail_save: true,
            ..Default::default()
        },
    );
    let mut events = service.event_tx.subscribe();

    complete_game(&service, EventTarget::Single(MachineId::new(1))).await;

    assert_eq!(
        events
            .try_recv()
            .expect("error event")
            .data
            .expect_err("save failed")
            .code(),
        tonic::Code::Internal
    );
}

#[tokio::test]
async fn get_game_result_reads_the_persisted_score_without_a_session() {
    let ds = RecordingDataSource::default();
    ds.saved.lock().push(crate::GameRecord {
        players: crate::GamePlayers::Single {
            machine_id: MachineId::new(1),
            player_id: PlayerId::new(11),
        },
        started_at: time::UtcDateTime::now(),
        difficulty: Difficulty::Normal,
        score: 123,
    });
    let service = GameMasterSessionServiceImpl::new(ds);

    let result = service
        .get_game_result(Request::new(GetGameResultRequest { game_id: 42 }))
        .await
        .expect("persisted result")
        .into_inner();

    assert_eq!(result, GetGameResultResponse { total_score: 123 });
}

#[tokio::test]
async fn get_game_result_returns_not_found_for_an_unknown_game_id() {
    let service = GameMasterSessionServiceImpl::new(RecordingDataSource::default());

    let error = service
        .get_game_result(Request::new(GetGameResultRequest { game_id: 999 }))
        .await
        .expect_err("unknown game");

    assert_eq!(error.code(), tonic::Code::NotFound);
}

#[tokio::test]
async fn get_game_result_reports_database_errors() {
    let service = GameMasterSessionServiceImpl::new(RecordingDataSource {
        fail_get_result: true,
        ..Default::default()
    });

    let error = service
        .get_game_result(Request::new(GetGameResultRequest { game_id: 42 }))
        .await
        .expect_err("database query fails");

    assert_eq!(error.code(), tonic::Code::Internal);
}

#[rstest]
#[case::single(GameMode::Single)]
#[case::multi(GameMode::Multi)]
#[tokio::test(start_paused = true)]
async fn pocket_watches_are_created_and_replaced_per_machine(#[case] mode: GameMode) {
    let service = configured_recorded_service(mode, RecordingDataSource::default());
    let mut events = service.event_tx.subscribe();
    let count = if mode.is_multi() { 2 } else { 1 };
    for machine_id in 1..=count {
        service
            .set_session_ready(Request::new(SetSessionReadyRequest { machine_id }))
            .await
            .expect("ready");
        if mode.is_multi() && machine_id == 1 {
            assert_matches!(
                events.try_recv(),
                Err(broadcast::error::TryRecvError::Empty)
            );
        }
    }
    let started = events.try_recv().expect("game started");
    match started.target {
        EventTarget::All => assert!(mode.is_multi()),
        EventTarget::Single(id) => assert_eq!(id, MachineId::new(1)),
    }
    assert_matches!(started.data, Ok(SuccessfulEvent::GameStarted { .. }));
    let mut watches = Vec::new();
    for _ in 0..count {
        let event = events.try_recv().expect("initial pocket watch");
        let EventTarget::Single(machine_id) = event.target else {
            panic!("single owner")
        };
        let Ok(SuccessfulEvent::TargetAdded { target }) = event.data else {
            panic!("target added")
        };
        assert_eq!(
            target.owner,
            Some(Owner::Single(struckout_proto::target::OwnerSingle {
                machine_id: machine_id.into(),
            }))
        );
        assert_matches!(
            target.target_data,
            Some(struckout_proto::target::TargetData::PocketWatch(_))
        );
        watches.push((machine_id, target.target_id));
    }
    assert_eq!(
        watches
            .iter()
            .map(|(id, _)| *id)
            .collect::<std::collections::HashSet<_>>()
            .len(),
        count as usize
    );
    assert_matches!(
        events.try_recv(),
        Err(broadcast::error::TryRecvError::Empty)
    );
    for (index, (machine_id, mut target_id)) in watches.into_iter().enumerate() {
        for hit in 0..2 {
            service
                .hit_target(Request::new(HitTargetRequest {
                    machine_id: machine_id.into(),
                    target_id: target_id.clone(),
                }))
                .await
                .expect("hit watch");
            let score = events.try_recv().expect("score event");
            assert_matches!(score.data, Ok(SuccessfulEvent::GameScoreChanged { total_score })
                if total_score == (index as u32 * 2 + hit + 1) * 500);
            match score.target {
                EventTarget::All => assert!(mode.is_multi()),
                EventTarget::Single(id) => assert_eq!(id, machine_id),
            }
            let removed = events.try_recv().expect("remove event");
            assert_matches!(removed.target, EventTarget::Single(id) if id == machine_id);
            assert_matches!(removed.data, Ok(SuccessfulEvent::TargetRemoved { target_id: id }) if id == target_id);
            let added = events.try_recv().expect("replacement event");
            assert_matches!(added.target, EventTarget::Single(id) if id == machine_id);
            let Ok(SuccessfulEvent::TargetAdded { target }) = added.data else {
                panic!("target added")
            };
            assert_ne!(target.target_id, target_id);
            assert_matches!(
                target.target_data,
                Some(struckout_proto::target::TargetData::PocketWatch(_))
            );
            let error = service
                .hit_target(Request::new(HitTargetRequest {
                    machine_id: machine_id.into(),
                    target_id,
                }))
                .await
                .expect_err("old watch removed");
            assert_eq!(error.code(), tonic::Code::NotFound);
            target_id = target.target_id;
            assert_matches!(
                events.try_recv(),
                Err(broadcast::error::TryRecvError::Empty)
            );
            let sessions = service.sessions.read();
            let (state,) = sessions
                .get(machine_id)
                .expect("session")
                .as_running()
                .expect("running");
            assert_eq!(state.shared.read().targets.len(), count as usize);
        }
    }
}
