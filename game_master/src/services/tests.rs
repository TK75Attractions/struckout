//! Tests of [`GameMasterSessionServiceImpl`].
use std::assert_matches;

use rstest::rstest;
use struckout_proto::{event::EventData, types::GameId};

use super::*;
use crate::{
    AddPlayerError, GamePlayerInfo, GetPlayerError, PlayerId, PlayerInfo, ValidatePlayerNameError,
    data::Player,
    session::{ConfiguringDifficultyState, ConfiguringGameModeState},
};

const TEST_GAME_START_DELAY_SECS: NonZeroU32 = NonZeroU32::new(1).expect("1 is nonzero");
const TEST_GAME_DURATION_SECS: NonZeroU32 = NonZeroU32::new(90).expect("90 is nonzero");
const TEST_GAME_DURATIONS: GameDurations = GameDurations {
    normal: TEST_GAME_DURATION_SECS,
    hard: TEST_GAME_DURATION_SECS,
    veryhard: TEST_GAME_DURATION_SECS,
};

fn test_combo_bonuses() -> ComboBonuses {
    #[derive(Deserialize)]
    struct TestConfig {
        combo_bonuses: ComboBonuses,
    }

    toml::from_str::<TestConfig>(
        "[[combo_bonuses]]\ncombo = 3\npoints = 500\n[[combo_bonuses]]\ncombo = 5\npoints = 800\n",
    )
    .expect("valid combo bonuses")
    .combo_bonuses
}

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

    async fn get_ranking(
        &self,
        _: u32,
        _: GameMode,
        _: Difficulty,
    ) -> Result<Vec<crate::GameResultItem>, crate::GetRankingError> {
        Ok(Vec::new())
    }

    async fn get_games_by_player(
        &self,
        _: PlayerId,
        _: GameMode,
        _: Difficulty,
    ) -> Result<Vec<crate::RankedGameResultItem>, crate::GetGamesByPlayerError> {
        Ok(Vec::new())
    }

    async fn get_player_by_id(
        &self,
        player_id: PlayerId,
    ) -> Result<Player, crate::GetPlayerByIdError> {
        Ok(Player {
            player_id,
            name: format!("Player {}", player_id.into_inner()),
        })
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
        loop {
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
                }) => break ret,
                Ok(ListenEventsResponse {
                    event:
                        Some(::struckout_proto::Event {
                            event_data: Some(EventData::GameWillStart(_)),
                            ..
                        }),
                }) => continue,
                el => panic!(
                    "received unexpected event: expected {}, actually {:?}",
                    stringify!($ev_pat),
                    el
                ),
            }
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
    let service = GameMasterSessionServiceImpl::new(
        StubDataSource { player_id },
        TEST_GAME_START_DELAY_SECS,
        TEST_GAME_DURATIONS,
        test_combo_bonuses(),
    );
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
    let service = GameMasterSessionServiceImpl::new(
        StubDataSource { player_id },
        TEST_GAME_START_DELAY_SECS,
        TEST_GAME_DURATIONS,
        test_combo_bonuses(),
    );
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
    let service = GameMasterSessionServiceImpl::new(
        StubDataSource {
            player_id: PlayerId::new(2),
        },
        TEST_GAME_START_DELAY_SECS,
        TEST_GAME_DURATIONS,
        test_combo_bonuses(),
    );
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
    let service = GameMasterSessionServiceImpl::new(
        ds.clone(),
        TEST_GAME_START_DELAY_SECS,
        TEST_GAME_DURATIONS,
        test_combo_bonuses(),
    );

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
async fn game_will_start_counts_down_before_game_started() {
    let service = GameMasterSessionServiceImpl::new(
        StubDataSource {
            player_id: PlayerId::new(1),
        },
        NonZeroU32::new(3).expect("3 is nonzero"),
        TEST_GAME_DURATIONS,
        test_combo_bonuses(),
    );
    let mut stream = service
        .listen_events(Request::new(ListenEventsRequest { machine_id: 1 }))
        .await
        .expect("listen")
        .into_inner();
    start_game_single(
        &service,
        MachineId::new(1),
        Difficulty::Normal,
        PlayerId::new(1),
    )
    .await;
    assert_event!(stream, EventData::SessionDifficultyChanged(_));
    for remaining in (1..=3).rev() {
        let response = stream
            .next()
            .await
            .expect("countdown event")
            .expect("event response");
        let EventData::GameWillStart(event) = response
            .event
            .expect("event")
            .event_data
            .expect("event data")
        else {
            panic!("expected GameWillStart");
        };
        assert_eq!(event.secs_until_start, remaining);
        tokio::time::advance(Duration::from_secs(1)).await;
    }
    assert_event!(stream, EventData::GameStarted(_));
}

#[tokio::test]
async fn set_session_difficulty_rejects_unspecified() {
    let service = GameMasterSessionServiceImpl::new(
        StubDataSource {
            player_id: PlayerId::new(1),
        },
        TEST_GAME_START_DELAY_SECS,
        TEST_GAME_DURATIONS,
        test_combo_bonuses(),
    );
    let error = service
        .set_session_difficulty(Request::new(SetSessionDifficultyRequest {
            difficulty: Difficulty::Unspecified.into(),
            machine_id: 1,
        }))
        .await
        .expect_err("unspecified has no configured duration");
    assert_eq!(error.code(), tonic::Code::InvalidArgument);
}

#[tokio::test(start_paused = true)]
async fn event_stream_notifies_in_correct_order() {
    let ds = StubDataSource {
        player_id: PlayerId::new(13),
    };
    let machine_id = MachineId::new(2);
    let difficulty = Difficulty::Normal;
    let service = GameMasterSessionServiceImpl::new(
        ds.clone(),
        TEST_GAME_START_DELAY_SECS,
        TEST_GAME_DURATIONS,
        test_combo_bonuses(),
    );

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

#[tokio::test(start_paused = true)]
async fn game_timer_uses_configured_duration() {
    let (event_tx, _) = broadcast::channel(32);
    let mut events = event_tx.subscribe();
    let (complete_tx, mut complete_rx) = mpsc::channel(1);
    let timer = tokio::spawn(game_timer(
        EventTarget::Single(MachineId::new(1)),
        NonZeroU32::new(3).expect("3 is nonzero"),
        event_tx,
        complete_tx,
    ));

    tokio::task::yield_now().await;
    assert_eq!(next_remaining_time(&mut events), 3);
    assert_matches!(
        events.try_recv(),
        Err(broadcast::error::TryRecvError::Empty)
    );
    assert_matches!(
        complete_rx.try_recv(),
        Err(mpsc::error::TryRecvError::Empty)
    );

    for remaining in (1..=2).rev() {
        tokio::time::advance(Duration::from_secs(1)).await;
        tokio::task::yield_now().await;
        assert_eq!(next_remaining_time(&mut events), remaining);
        assert_matches!(
            events.try_recv(),
            Err(broadcast::error::TryRecvError::Empty)
        );
        assert_matches!(
            complete_rx.try_recv(),
            Err(mpsc::error::TryRecvError::Empty)
        );
    }

    tokio::time::advance(Duration::from_secs(1)).await;
    timer.await.expect("timer completes");
    complete_rx
        .recv()
        .await
        .expect("game completes at 3 seconds");
}

#[rstest]
#[case::single_normal(GameMode::Single, Difficulty::Normal, 4)]
#[case::single_hard(GameMode::Single, Difficulty::Hard, 3)]
#[case::single_veryhard(GameMode::Single, Difficulty::Veryhard, 2)]
#[case::multi_normal(GameMode::Multi, Difficulty::Normal, 4)]
#[case::multi_hard(GameMode::Multi, Difficulty::Hard, 3)]
#[case::multi_veryhard(GameMode::Multi, Difficulty::Veryhard, 2)]
#[tokio::test(start_paused = true)]
async fn game_duration_follows_selected_difficulty(
    #[case] mode: GameMode,
    #[case] difficulty: Difficulty,
    #[case] expected_secs: u32,
) {
    let mut service = configured_recorded_service(mode, RecordingDataSource::default());
    service.game_duration_secs = GameDurations {
        normal: NonZeroU32::new(4).expect("4 is nonzero"),
        hard: NonZeroU32::new(3).expect("3 is nonzero"),
        veryhard: NonZeroU32::new(2).expect("2 is nonzero"),
    };
    service
        .sessions
        .write()
        .update_difficulty(MachineId::new(1), difficulty)
        .expect("configure difficulty");
    let mut stream = service
        .listen_events(Request::new(ListenEventsRequest { machine_id: 1 }))
        .await
        .expect("subscribe before the game starts")
        .into_inner();

    for machine_id in 1..=if mode.is_multi() { 2 } else { 1 } {
        service
            .set_session_ready(Request::new(SetSessionReadyRequest { machine_id }))
            .await
            .expect("start game");
    }

    let EventData::GameStarted(started) = assert_event!(stream, EventData::GameStarted(_)) else {
        unreachable!("assert_event matches GameStarted")
    };
    assert_eq!(started.game_dur_secs, expected_secs);

    loop {
        let event = stream
            .next()
            .await
            .expect("event stream remains open")
            .expect("event delivery succeeds")
            .event
            .expect("event is present")
            .event_data
            .expect("event data is present");
        if let EventData::GameTimeLimitNotify(notify) = event {
            assert_eq!(
                notify.remaining.expect("remaining time is present").seconds,
                i64::from(expected_secs)
            );
            break;
        }
    }
}

fn next_remaining_time(events: &mut broadcast::Receiver<Event>) -> i64 {
    let event = events
        .try_recv()
        .expect("remaining-time event is available");
    let SuccessfulEvent::GameTimeLimitNotify { remaining } =
        event.data.expect("timer event succeeds")
    else {
        panic!("expected remaining-time event");
    };
    remaining.whole_seconds()
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
    let service = GameMasterSessionServiceImpl::new(
        StubDataSource {
            player_id: PlayerId::new(1),
        },
        TEST_GAME_START_DELAY_SECS,
        TEST_GAME_DURATIONS,
        test_combo_bonuses(),
    );
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
    let service = GameMasterSessionServiceImpl::new(
        StubDataSource {
            player_id: PlayerId::new(1),
        },
        TEST_GAME_START_DELAY_SECS,
        TEST_GAME_DURATIONS,
        test_combo_bonuses(),
    );
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
    let service = GameMasterSessionServiceImpl::new(
        StubDataSource {
            player_id: PlayerId::new(1),
        },
        TEST_GAME_START_DELAY_SECS,
        TEST_GAME_DURATIONS,
        test_combo_bonuses(),
    );
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
    let service = GameMasterSessionServiceImpl::new(
        StubDataSource {
            player_id: PlayerId::new(1),
        },
        TEST_GAME_START_DELAY_SECS,
        TEST_GAME_DURATIONS,
        test_combo_bonuses(),
    );
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
    let service = GameMasterSessionServiceImpl::new(
        StubDataSource {
            player_id: PlayerId::new(1),
        },
        TEST_GAME_START_DELAY_SECS,
        TEST_GAME_DURATIONS,
        test_combo_bonuses(),
    );
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
            total_score: 100_000,
            diff: 100_000,
            reason: super::ScoreReason::TargetHit { target_id: id },
        }) if id == target_id
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
    let service = GameMasterSessionServiceImpl::new(
        StubDataSource {
            player_id: PlayerId::new(1),
        },
        TEST_GAME_START_DELAY_SECS,
        TEST_GAME_DURATIONS,
        test_combo_bonuses(),
    );
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
    let service = GameMasterSessionServiceImpl::new(
        StubDataSource {
            player_id: PlayerId::new(1),
        },
        TEST_GAME_START_DELAY_SECS,
        TEST_GAME_DURATIONS,
        test_combo_bonuses(),
    );
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
    fail_get_player: bool,
    player_lookup_started: Arc<tokio::sync::Notify>,
    player_lookup_gate: Option<Arc<tokio::sync::Semaphore>>,
    ranking: Vec<crate::GameResultItem>,
    history: Vec<crate::RankedGameResultItem>,
    ranking_limits: Arc<parking_lot::Mutex<Vec<(u32, GameMode, Difficulty)>>>,
    history_players: Arc<parking_lot::Mutex<Vec<(PlayerId, GameMode, Difficulty)>>>,
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

    async fn get_ranking(
        &self,
        max_item_num: u32,
        mode: GameMode,
        difficulty: Difficulty,
    ) -> Result<Vec<crate::GameResultItem>, crate::GetRankingError> {
        self.ranking_limits
            .lock()
            .push((max_item_num, mode, difficulty));
        if self.fail_get_result {
            return Err(sqlx::Error::PoolClosed.into());
        }
        Ok(self.ranking.clone())
    }

    async fn get_games_by_player(
        &self,
        player_id: PlayerId,
        mode: GameMode,
        difficulty: Difficulty,
    ) -> Result<Vec<crate::RankedGameResultItem>, crate::GetGamesByPlayerError> {
        self.history_players
            .lock()
            .push((player_id, mode, difficulty));
        if self.fail_get_result {
            return Err(sqlx::Error::PoolClosed.into());
        }
        Ok(self.history.clone())
    }

    async fn get_player_by_id(
        &self,
        player_id: PlayerId,
    ) -> Result<Player, crate::GetPlayerByIdError> {
        self.player_lookup_started.notify_one();
        if let Some(gate) = &self.player_lookup_gate {
            gate.acquire()
                .await
                .expect("lookup gate remains open")
                .forget();
        }
        if self.fail_get_player {
            return Err(sqlx::Error::PoolClosed.into());
        }
        Ok(Player {
            player_id,
            name: format!("Player {}", player_id.into_inner()),
        })
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
    let service = GameMasterSessionServiceImpl::new(
        data_source,
        TEST_GAME_START_DELAY_SECS,
        TEST_GAME_DURATIONS,
        test_combo_bonuses(),
    );
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

/// Collects start notifications until the stream has no events for 10 milliseconds.
async fn collect_game_started(
    stream: &mut <GameMasterSessionServiceImpl<RecordingDataSource> as GameMasterSessionService>::ListenEventsStream,
) -> Vec<struckout_proto::event::GameStarted> {
    let mut started = Vec::new();
    while let Ok(event) = tokio::time::timeout(Duration::from_millis(10), stream.next()).await {
        let event = event
            .expect("event stream remains open")
            .expect("event delivery succeeds")
            .event
            .expect("event is present")
            .event_data
            .expect("event data is present");
        if let EventData::GameStarted(event) = event {
            started.push(event);
        }
    }
    started
}

#[rstest]
#[case::machine_1_first([1, 2])]
#[case::machine_2_first([2, 1])]
#[tokio::test(start_paused = true)]
async fn multi_game_started_reaches_both_machines_once(#[case] ready_order: [u32; 2]) {
    use struckout_proto::{event::GameStarted, player_info};

    let service = configured_recorded_service(GameMode::Multi, RecordingDataSource::default());
    let mut streams = Vec::new();
    for machine_id in [1, 2] {
        streams.push(
            service
                .listen_events(Request::new(ListenEventsRequest { machine_id }))
                .await
                .expect("subscribe before either machine is ready")
                .into_inner(),
        );
    }

    service
        .set_session_ready(Request::new(SetSessionReadyRequest {
            machine_id: ready_order[0],
        }))
        .await
        .expect("first machine waits for its partner");
    for stream in &mut streams {
        assert!(collect_game_started(stream).await.is_empty());
    }

    service
        .set_session_ready(Request::new(SetSessionReadyRequest {
            machine_id: ready_order[1],
        }))
        .await
        .expect("second machine starts the shared game");
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_secs(1)).await;

    let player_info = |machine_id| {
        Some(player_info::PlayerItem {
            machine_id,
            player_id: machine_id + 10,
            player_name: format!("Player {}", machine_id + 10),
        })
    };
    let expected = GameStarted {
        game_dur_secs: TEST_GAME_DURATION_SECS.get(),
        difficulty: Difficulty::Normal.into(),
        player_info: Some(struckout_proto::PlayerInfo {
            player_info_data: Some(player_info::PlayerInfoData::Multi(player_info::Multi {
                player_1: player_info(ready_order[1]),
                player_2: player_info(ready_order[0]),
            })),
        }),
    };
    for stream in &mut streams {
        assert_eq!(collect_game_started(stream).await, vec![expected.clone()]);
    }
}

#[rstest]
#[case::machine_1(1, 2)]
#[case::machine_2(2, 1)]
#[tokio::test(start_paused = true)]
async fn single_game_started_reaches_only_its_machine(
    #[case] machine_id: u32,
    #[case] other_machine_id: u32,
) {
    use struckout_proto::{event::GameStarted, player_info};

    let service = GameMasterSessionServiceImpl::new(
        RecordingDataSource::default(),
        TEST_GAME_START_DELAY_SECS,
        GameDurations {
            normal: NonZeroU32::new(30).expect("30 is nonzero"),
            ..TEST_GAME_DURATIONS
        },
        test_combo_bonuses(),
    );
    let mut streams = Vec::new();
    for machine_id in [machine_id, other_machine_id] {
        streams.push(
            service
                .listen_events(Request::new(ListenEventsRequest { machine_id }))
                .await
                .expect("subscribe before starting the game")
                .into_inner(),
        );
    }

    start_game_single(
        &service,
        MachineId::new(machine_id),
        Difficulty::Normal,
        PlayerId::new(11),
    )
    .await;
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_secs(1)).await;

    assert_eq!(
        collect_game_started(&mut streams[0]).await,
        vec![GameStarted {
            game_dur_secs: 30,
            difficulty: Difficulty::Normal.into(),
            player_info: Some(struckout_proto::PlayerInfo {
                player_info_data: Some(player_info::PlayerInfoData::Single(player_info::Single {
                    player: Some(player_info::PlayerItem {
                        machine_id,
                        player_id: 11,
                        player_name: "Player 11".to_owned(),
                    }),
                },))
            }),
        }]
    );
    assert!(collect_game_started(&mut streams[1]).await.is_empty());
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
    let service = GameMasterSessionServiceImpl::new(
        ds,
        TEST_GAME_START_DELAY_SECS,
        TEST_GAME_DURATIONS,
        test_combo_bonuses(),
    );

    let result = service
        .get_game_result(Request::new(GetGameResultRequest { game_id: 42 }))
        .await
        .expect("persisted result")
        .into_inner();

    assert_eq!(result, GetGameResultResponse { total_score: 123 });
}

#[tokio::test]
async fn get_game_result_returns_not_found_for_an_unknown_game_id() {
    let service = GameMasterSessionServiceImpl::new(
        RecordingDataSource::default(),
        TEST_GAME_START_DELAY_SECS,
        TEST_GAME_DURATIONS,
        test_combo_bonuses(),
    );

    let error = service
        .get_game_result(Request::new(GetGameResultRequest { game_id: 999 }))
        .await
        .expect_err("unknown game");

    assert_eq!(error.code(), tonic::Code::NotFound);
}

#[tokio::test]
async fn get_game_result_reports_database_errors() {
    let service = GameMasterSessionServiceImpl::new(
        RecordingDataSource {
            fail_get_result: true,
            ..Default::default()
        },
        TEST_GAME_START_DELAY_SECS,
        TEST_GAME_DURATIONS,
        test_combo_bonuses(),
    );

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
    tokio::task::yield_now().await;
    let countdown = events.try_recv().expect("countdown");
    assert_matches!(
        countdown.data,
        Ok(SuccessfulEvent::GameWillStart {
            secs_until_start: 1
        })
    );
    tokio::time::advance(Duration::from_secs(1)).await;
    tokio::task::yield_now().await;
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
        Ok(Event {
            data: Ok(SuccessfulEvent::GameTimeLimitNotify { .. }),
            ..
        })
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
            let hit_number = index as u32 * 2 + hit + 1;
            let bonus = if hit_number == 3 { 500 } else { 0 };
            assert_matches!(score.data, Ok(SuccessfulEvent::GameScoreChanged { total_score, diff: 500, reason: super::ScoreReason::TargetHit { target_id: id } })
                if id == target_id && total_score == hit_number * 500 + if hit_number > 3 { 500 } else { 0 });
            if bonus > 0 {
                let bonus_event = events.try_recv().expect("combo bonus event");
                assert_matches!(bonus_event.data, Ok(SuccessfulEvent::GameScoreChanged { total_score, diff: 500, reason: super::ScoreReason::Combo })
                    if total_score == hit_number * 500 + bonus);
            }
            match score.target {
                EventTarget::All => assert!(mode.is_multi()),
                EventTarget::Single(id) => assert_eq!(id, machine_id),
            }
            let combo = events.try_recv().expect("combo event");
            assert_matches!(combo.data, Ok(SuccessfulEvent::ComboChanged { combo })
                if combo == index as u32 * 2 + hit + 1);
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

#[rstest]
#[case::empty(false, false)]
#[case::single(true, false)]
#[case::multi(true, true)]
#[tokio::test]
async fn ranking_and_history_read_persisted_data_without_a_session(
    #[case] populated: bool,
    #[case] multi: bool,
) {
    let first = PlayerInfo {
        machine_id: MachineId::new(3),
        player_id: PlayerId::new(7),
        player_name: "太郎".to_owned(),
    };
    let players = if multi {
        GamePlayerInfo::Multi(
            first,
            PlayerInfo {
                machine_id: MachineId::new(8),
                player_id: PlayerId::new(9),
                player_name: "花子".to_owned(),
            },
        )
    } else {
        GamePlayerInfo::Single(first)
    };
    let mode = if multi {
        GameMode::Multi
    } else {
        GameMode::Single
    };
    let result = crate::GameResult {
        game_id: GameId::new(42),
        total_score: 987,
    };
    let ds = RecordingDataSource {
        ranking: if populated {
            vec![crate::GameResultItem {
                result: result.clone(),
                players: players.clone(),
            }]
        } else {
            Vec::new()
        },
        history: if populated {
            vec![crate::RankedGameResultItem {
                item: crate::GameResultItem {
                    result,
                    players: players.clone(),
                },
                rank: 3,
            }]
        } else {
            Vec::new()
        },
        ..Default::default()
    };
    let service = GameMasterSessionServiceImpl::new(
        ds.clone(),
        TEST_GAME_START_DELAY_SECS,
        TEST_GAME_DURATIONS,
        test_combo_bonuses(),
    );
    let ranking = service
        .get_ranking(Request::new(GetRankingRequest {
            max_item_num: 10,
            mode: mode.into(),
            difficulty: Difficulty::Normal.into(),
        }))
        .await
        .expect("ranking should succeed")
        .into_inner()
        .top_n;
    let history = service
        .get_games_by_player(Request::new(GetGamesByPlayerRequest {
            player_id: 7,
            mode: mode.into(),
            difficulty: Difficulty::Normal.into(),
        }))
        .await
        .expect("history should succeed")
        .into_inner()
        .items;
    assert_eq!(
        *ds.ranking_limits.lock(),
        vec![(10, mode, Difficulty::Normal)]
    );
    assert_eq!(
        *ds.history_players.lock(),
        vec![(PlayerId::new(7), mode, Difficulty::Normal)]
    );
    assert_eq!(ranking.len(), usize::from(populated));
    assert_eq!(history.len(), usize::from(populated));
    if populated {
        assert_eq!(
            ranking[0],
            struckout_proto::get_ranking_response::RankingItem {
                game_id: 42,
                total_score: 987,
                player_info: Some(players.clone().into()),
            }
        );
        assert_eq!(
            history[0],
            struckout_proto::get_games_by_player_response::GamesByPlayerItem {
                game_id: 42,
                total_score: 987,
                player_info: Some(players.into()),
                rank: 3,
            }
        );
    }
}

#[tokio::test]
async fn ranking_and_history_report_database_errors() {
    let service = GameMasterSessionServiceImpl::new(
        RecordingDataSource {
            fail_get_result: true,
            ..Default::default()
        },
        TEST_GAME_START_DELAY_SECS,
        TEST_GAME_DURATIONS,
        test_combo_bonuses(),
    );
    let error = service
        .get_ranking(Request::new(GetRankingRequest {
            max_item_num: 10,
            mode: GameMode::Single.into(),
            difficulty: Difficulty::Normal.into(),
        }))
        .await
        .expect_err("database is closed");
    assert_eq!(error.code(), tonic::Code::Internal);
    let error = service
        .get_games_by_player(Request::new(GetGamesByPlayerRequest {
            player_id: 7,
            mode: GameMode::Single.into(),
            difficulty: Difficulty::Normal.into(),
        }))
        .await
        .expect_err("database is closed");
    assert_eq!(error.code(), tonic::Code::Internal);
}

#[rstest]
#[case(0, 1)]
#[case(1, 0)]
#[case(-1, 1)]
#[case(1, -1)]
#[case(i32::MAX, 1)]
#[case(1, i32::MAX)]
#[tokio::test]
async fn result_queries_reject_invalid_filters(#[case] mode: i32, #[case] difficulty: i32) {
    let ds = RecordingDataSource::default();
    let service = GameMasterSessionServiceImpl::new(
        ds.clone(),
        TEST_GAME_START_DELAY_SECS,
        TEST_GAME_DURATIONS,
        test_combo_bonuses(),
    );
    let error = service
        .get_ranking(Request::new(GetRankingRequest {
            max_item_num: 0,
            mode,
            difficulty,
        }))
        .await
        .expect_err("filter must be valid even for an empty ranking");
    assert_eq!(error.code(), tonic::Code::InvalidArgument);
    let error = service
        .get_games_by_player(Request::new(GetGamesByPlayerRequest {
            player_id: 1,
            mode,
            difficulty,
        }))
        .await
        .expect_err("filter must be valid");
    assert_eq!(error.code(), tonic::Code::InvalidArgument);
    assert!(ds.ranking_limits.lock().is_empty());
    assert!(ds.history_players.lock().is_empty());
}

#[tokio::test]
async fn player_lookup_failure_leaves_session_configuration_unchanged() {
    let service = configured_recorded_service(
        GameMode::Single,
        RecordingDataSource {
            fail_get_player: true,
            ..Default::default()
        },
    );
    let mut events = service.event_tx.subscribe();
    let error = service
        .set_session_ready(Request::new(SetSessionReadyRequest { machine_id: 1 }))
        .await
        .expect_err("database lookup failed");
    assert_eq!(error.code(), tonic::Code::Internal);
    assert!(
        service
            .sessions
            .read()
            .get(MachineId::new(1))
            .expect("session exists")
            .as_configuring_difficulty()
            .is_ok()
    );
    assert_matches!(
        events.try_recv(),
        Err(broadcast::error::TryRecvError::Empty)
    );
}

#[tokio::test]
async fn ready_rejects_configuration_changes_during_player_lookup() {
    let gate = Arc::new(tokio::sync::Semaphore::new(0));
    let ds = RecordingDataSource {
        player_lookup_gate: Some(gate.clone()),
        ..Default::default()
    };
    let service = configured_recorded_service(GameMode::Single, ds.clone());
    let task = tokio::spawn({
        let service = service.clone();
        async move {
            service
                .set_session_ready(Request::new(SetSessionReadyRequest { machine_id: 1 }))
                .await
        }
    });
    ds.player_lookup_started.notified().await;
    service
        .sessions
        .write()
        .update_difficulty(MachineId::new(1), Difficulty::Hard)
        .expect("configuration can change during lookup");
    gate.add_permits(1);
    let error = task
        .await
        .expect("ready task completes")
        .expect_err("configuration has changed");
    assert_eq!(error.code(), tonic::Code::Aborted);
    let sessions = service.sessions.read();
    let (state,) = sessions
        .get(MachineId::new(1))
        .expect("session exists")
        .as_configuring_difficulty()
        .expect("game was not started");
    assert_eq!(state.difficulty, Some(Difficulty::Hard));
}

#[rstest]
#[case::single(GameMode::Single, EventTarget::Single(MachineId::new(1)))]
#[case::multi(GameMode::Multi, EventTarget::All)]
#[tokio::test]
async fn home_navigation_after_completion_only_notifies_the_requested_machine(
    #[case] mode: GameMode,
    #[case] target: EventTarget,
) {
    let service = running_recorded_service(mode, RecordingDataSource::default());
    let mut first = service
        .listen_events(Request::new(ListenEventsRequest { machine_id: 1 }))
        .await
        .expect("subscribe first machine")
        .into_inner();
    let mut second = service
        .listen_events(Request::new(ListenEventsRequest { machine_id: 2 }))
        .await
        .expect("subscribe second machine")
        .into_inner();
    complete_game(&service, target).await;
    assert_event!(first, EventData::GameFinished(_));
    if mode.is_multi() {
        assert_event!(second, EventData::GameFinished(_));
    }
    for machine_id in [1, 2] {
        assert!(service.sessions.read().get(machine_id.into()).is_none());
    }

    for machine_id in [1, 2] {
        service
            .go_home_screen(Request::new(GoHomeScreenRequest { machine_id }))
            .await
            .expect("request home after session disposal");
    }
    for (machine_id, mut stream) in [(1, first), (2, second)] {
        let EventData::NavigateToHomeScreenRequsted(event) =
            assert_event!(stream, EventData::NavigateToHomeScreenRequsted(_))
        else {
            unreachable!("event pattern was checked above")
        };
        assert_eq!(event.machine_id, machine_id);
        assert!(
            tokio::time::timeout(Duration::from_millis(10), stream.next())
                .await
                .is_err()
        );
    }
}

#[tokio::test]
async fn home_navigation_can_be_retried_after_subscribing_without_a_session() {
    let service = GameMasterSessionServiceImpl::new(
        StubDataSource {
            player_id: PlayerId::new(2),
        },
        TEST_GAME_START_DELAY_SECS,
        TEST_GAME_DURATIONS,
        test_combo_bonuses(),
    );
    let error = service
        .go_home_screen(Request::new(GoHomeScreenRequest { machine_id: 1 }))
        .await
        .expect_err("no event subscribers");
    assert_eq!(error.code(), tonic::Code::Unavailable);

    let mut stream = service
        .listen_events(Request::new(ListenEventsRequest { machine_id: 1 }))
        .await
        .expect("subscribe before retrying")
        .into_inner();
    for _ in 0..2 {
        service
            .go_home_screen(Request::new(GoHomeScreenRequest { machine_id: 1 }))
            .await
            .expect("retry home without a session");
        assert_event!(
            stream,
            EventData::NavigateToHomeScreenRequsted(
                struckout_proto::event::NavigateToHomeScreenRequested { machine_id: 1 }
            )
        );
    }
    assert!(service.sessions.read().get(MachineId::new(1)).is_none());
}

#[rstest]
#[case::single(GameMode::Single)]
#[case::multi(GameMode::Multi)]
#[tokio::test]
async fn combo_events_bonuses_reset_and_saved_result(#[case] mode: GameMode) {
    let ds = RecordingDataSource::default();
    let service = running_recorded_service(mode, ds.clone());
    let target_id = service
        .add_target(Request::new(target_request(1)))
        .await
        .expect("register target")
        .into_inner()
        .target_id;
    let mut first = service
        .listen_events(Request::new(ListenEventsRequest { machine_id: 1 }))
        .await
        .expect("first listener")
        .into_inner();
    let mut second = service
        .listen_events(Request::new(ListenEventsRequest { machine_id: 2 }))
        .await
        .expect("second listener")
        .into_inner();
    service
        .reset_combo(Request::new(ResetComboRequest { machine_id: 1 }))
        .await
        .expect("initial zero combo");
    let mut total = 123;
    for combo in 1..=6 {
        let machine_id = if mode.is_multi() && combo % 2 == 0 {
            2
        } else {
            1
        };
        service
            .hit_target(Request::new(HitTargetRequest {
                machine_id,
                target_id: target_id.clone(),
            }))
            .await
            .expect("hit target");
        total += 100_000
            + match combo {
                3 => 500,
                5 => 800,
                _ => 0,
            };
        let bonus = match combo {
            3 => 500,
            5 => 800,
            _ => 0,
        };
        let score = assert_event!(first, EventData::GameScoreChanged(_));
        assert_matches!(score, EventData::GameScoreChanged(event)
            if event.total_score == total - bonus && event.diff == 100_000
                && matches!(event.score_reason, Some(struckout_proto::event::game_score_changed::ScoreReason::TargetHit(_))));
        if bonus > 0 {
            let score = assert_event!(first, EventData::GameScoreChanged(_));
            assert_matches!(score, EventData::GameScoreChanged(event)
                if event.total_score == total && event.diff == i64::from(bonus)
                    && matches!(event.score_reason, Some(struckout_proto::event::game_score_changed::ScoreReason::Combo(()))));
        }
        let changed = assert_event!(first, EventData::ComboChanged(_));
        assert_matches!(changed, EventData::ComboChanged(event) if event.combo == combo);
        if mode.is_multi() {
            assert_event!(second, EventData::GameScoreChanged(_));
            if bonus > 0 {
                assert_event!(second, EventData::GameScoreChanged(_));
            }
            let changed = assert_event!(second, EventData::ComboChanged(_));
            assert_matches!(changed, EventData::ComboChanged(event) if event.combo == combo);
        }
    }
    service
        .reset_combo(Request::new(ResetComboRequest { machine_id: 1 }))
        .await
        .expect("reset combo");
    let changed = assert_event!(first, EventData::ComboChanged(_));
    assert_matches!(changed, EventData::ComboChanged(event) if event.combo == 0);
    if mode.is_multi() {
        assert_event!(second, EventData::ComboChanged(_));
    }
    service
        .reset_combo(Request::new(ResetComboRequest { machine_id: 1 }))
        .await
        .expect("already zero");
    service
        .hit_target(Request::new(HitTargetRequest {
            machine_id: 1,
            target_id,
        }))
        .await
        .expect("first hit after reset");
    total += 100_000;
    assert_event!(first, EventData::GameScoreChanged(_));
    let changed = assert_event!(first, EventData::ComboChanged(_));
    assert_matches!(changed, EventData::ComboChanged(event) if event.combo == 1);
    if mode.is_multi() {
        assert_event!(second, EventData::GameScoreChanged(_));
        assert_event!(second, EventData::ComboChanged(_));
    } else {
        assert!(
            tokio::time::timeout(Duration::from_millis(10), second.next())
                .await
                .is_err()
        );
    }
    let target = if mode.is_multi() {
        EventTarget::All
    } else {
        EventTarget::Single(MachineId::new(1))
    };
    complete_game(&service, target).await;
    assert_eq!(ds.saved.lock()[0].score, total);
    assert_eq!(
        service
            .get_game_result(Request::new(GetGameResultRequest { game_id: 42 }))
            .await
            .expect("persisted result")
            .into_inner()
            .total_score,
        total
    );
}

#[rstest]
#[case::unknown(99, false)]
#[case::not_running(1, true)]
#[tokio::test]
async fn reset_combo_rejects_invalid_machine_or_state(
    #[case] machine_id: u32,
    #[case] configure: bool,
) {
    let service = GameMasterSessionServiceImpl::new(
        StubDataSource {
            player_id: PlayerId::new(1),
        },
        TEST_GAME_START_DELAY_SECS,
        TEST_GAME_DURATIONS,
        test_combo_bonuses(),
    );
    if configure {
        service
            .start_session(Request::new(StartSessionRequest {
                machine_id,
                player_id: 1,
            }))
            .await
            .expect("new session");
    }
    let mut events = service.event_tx.subscribe();
    let error = service
        .reset_combo(Request::new(ResetComboRequest { machine_id }))
        .await
        .expect_err("machine is not playing");
    assert_eq!(error.code(), tonic::Code::FailedPrecondition);
    assert_matches!(
        events.try_recv(),
        Err(broadcast::error::TryRecvError::Empty)
    );
}
