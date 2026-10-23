mod player;
pub use player::GameMasterPlayerServiceImpl;

use std::{pin::Pin, sync::Arc, time::Duration};

use parking_lot::RwLock;
use struckout_proto::{
    self, AddScoreRequest, AddScoreResponse, Difficulty, GetGameResultRequest,
    GetGameResultResponse, GoNextTutorialRequest, GoNextTutorialResponse,
    GoPreviousTutorialRequest, GoPreviousTutorialResponse, ListenEventsRequest,
    ListenEventsResponse, SetGameModeRequest, SetGameModeResponse, SetSessionDifficultyRequest,
    SetSessionDifficultyResponse, SetSessionReadyRequest, SetSessionReadyResponse,
    StartSessionRequest, StartSessionResponse, StartTutorialRequest, StartTutorialResponse,
    game_master_session_service_server::GameMasterSessionService,
    types::{GameId, MachineId, PlayerId},
};
use time::{SignedDuration, UtcDateTime, ext::NumericalDuration};
use tokio::sync::{broadcast, mpsc};
use tokio_stream::{
    Stream, StreamExt,
    wrappers::{BroadcastStream, ReceiverStream},
};
use tonic::{Request, Response, Status};
use tracing::{instrument, trace, warn};

use crate::{
    DataSource, DataSourceImpl, GetGameResultError,
    session::{Sessions, SetReadyAction},
};

const GAME_DURATION: SignedDuration = SignedDuration::seconds(90);

/// Implementation of [`GameMasterSessionService`].
#[derive(derive_more::Debug, Clone)]
pub struct GameMasterSessionServiceImpl<D = DataSourceImpl> {
    sessions: Arc<RwLock<Sessions>>,
    #[debug(skip)]
    data_source: D,
    event_tx: broadcast::Sender<Event>,
}

impl<D> GameMasterSessionServiceImpl<D>
where
    D: DataSource,
{
    pub fn new(data_source: D) -> Self {
        let (event_tx, _) = broadcast::channel(32);
        Self {
            sessions: Default::default(),
            data_source,
            event_tx,
        }
    }

    /// Handles completion of game when notified via `complete_rx`.
    async fn handle_game_completion(
        self,
        game_id: GameId,
        ev_target: EventTarget,
        mut complete_rx: mpsc::Receiver<()>,
    ) {
        complete_rx.recv().await.expect("complete channel closed");
        let score = {
            let sessions = self.sessions.read();
            match ev_target {
                EventTarget::Single(machine_id) => sessions
                    .score(machine_id)
                    .expect("it is guaranteed the session exist and is in right state"),
                EventTarget::All => sessions.score_multi().expect("should be in valid state"),
            }
        };
        let res = self.data_source.complete_game(game_id, score).await;
        if let Err(e) = res {
            self.event_tx
                .send(Event {
                    game_id,
                    target: ev_target,
                    data: Err(Status::internal(e.to_string())),
                })
                .expect("all event receiver has been dropped");
        };
        {
            let mut sessions = self.sessions.write();
            match ev_target {
                EventTarget::Single(machine_id) => {
                    let removed = sessions.remove(machine_id);
                    assert!(removed.is_some());
                }
                EventTarget::All => {
                    let (removed1, removed2) = sessions.remove_all();
                    assert!(removed1.is_some());
                    assert!(removed2.is_some());
                }
            }
        }
        self.event_tx
            .send(Event {
                game_id,
                target: ev_target,
                data: Ok(SuccessfulEvent::GameFinished),
            })
            .expect("all event receiver has been dropped");
    }
}

#[tonic::async_trait]
impl<D> GameMasterSessionService for GameMasterSessionServiceImpl<D>
where
    D: DataSource,
{
    #[instrument]
    async fn start_session(
        &self,
        req: Request<StartSessionRequest>,
    ) -> Result<Response<StartSessionResponse>, Status> {
        trace!(
            remote_addr = ?req.remote_addr(),
            "received start_session request"
        );
        let req = req.into_inner();
        let machine_id: MachineId = req.machine_id.into();
        let player_id: PlayerId = req.player_id.into();
        {
            let mut sessions = self.sessions.write();
            sessions
                .insert(machine_id, player_id)
                .map_err(|e| Status::failed_precondition(e.to_string()))?;
        }

        Ok(Response::new(StartSessionResponse {}))
    }

    async fn set_session_difficulty(
        &self,
        req: Request<SetSessionDifficultyRequest>,
    ) -> Result<Response<SetSessionDifficultyResponse>, Status> {
        let req = req.into_inner();
        let SetSessionDifficultyRequest {
            difficulty,
            machine_id,
        } = req;
        let difficulty = Difficulty::try_from(difficulty)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;
        let machine_id: MachineId = machine_id.into();

        {
            let mut sessions = self.sessions.write();
            sessions
                .update_difficulty(machine_id, difficulty)
                .map_err(|e| Status::failed_precondition(e.to_string()))?;
        }
        todo!("notify change")
    }

    async fn set_game_mode(
        &self,
        _req: Request<SetGameModeRequest>,
    ) -> Result<Response<SetGameModeResponse>, Status> {
        todo!()
    }

    async fn start_tutorial(
        &self,
        _req: Request<StartTutorialRequest>,
    ) -> Result<Response<StartTutorialResponse>, Status> {
        todo!()
    }

    async fn go_previous_tutorial(
        &self,
        _req: Request<GoPreviousTutorialRequest>,
    ) -> Result<Response<GoPreviousTutorialResponse>, Status> {
        todo!()
    }

    async fn go_next_tutorial(
        &self,
        _req: Request<GoNextTutorialRequest>,
    ) -> Result<Response<GoNextTutorialResponse>, Status> {
        todo!()
    }

    type ListenEventsStream =
        Pin<Box<dyn Stream<Item = Result<ListenEventsResponse, Status>> + Send>>;

    async fn listen_events(
        &self,
        req: Request<ListenEventsRequest>,
    ) -> Result<Response<Self::ListenEventsStream>, Status> {
        let req = req.into_inner();
        let machine_id: MachineId = req.machine_id.into();
        let (stream_tx, stream_rx) = mpsc::channel(32);
        let event_rx = self.event_tx.subscribe();
        tokio::spawn(pass_events_through_by_machine_id(
            machine_id, event_rx, stream_tx,
        ));

        let out_stream = ReceiverStream::new(stream_rx);
        Ok(Response::new(
            Box::pin(out_stream) as Self::ListenEventsStream
        ))
    }

    async fn add_score(
        &self,
        req: Request<AddScoreRequest>,
    ) -> Result<Response<AddScoreResponse>, Status> {
        let req = req.into_inner();
        let game_id = req.game_id.into();
        let machine_id = req.machine_id.into();
        {
            let mut sessions = self.sessions.write();
        }
        let total_score = {
            let mut guard = self.running_games.write();
            let Some(game) = guard.get_mut(&game_id) else {
                return Err(Status::not_found(format!(
                    "game with id {} does not exist or is not running",
                    game_id.into_inner()
                )));
            };
            game.score += req.score_to_add;
            game.score
        };
        self.event_tx
            .send(Event {
                game_id,
                machine_id,
                data: Ok(SuccessfulEvent::GameScoreChanged { total_score }),
            })
            .unwrap();
        Ok(Response::new(AddScoreResponse {}))
    }

    async fn get_game_result(
        &self,
        req: Request<GetGameResultRequest>,
    ) -> Result<Response<GetGameResultResponse>, Status> {
        let req = req.into_inner();
        let game_id = req.game_id.into();
        match self.data_source.get_game_result(game_id).await {
            Ok(record) => Ok(Response::new(GetGameResultResponse {
                total_score: record.score,
            })),
            Err(e @ GetGameResultError::GameNotFound) => Err(Status::not_found(e.to_string())),
            Err(e @ GetGameResultError::NotYetCompleted) => Err(Status::aborted(e.to_string())),
            Err(GetGameResultError::Sqlx(e)) => Err(Status::internal(e.to_string())),
        }
    }

    #[instrument]
    async fn set_session_ready(
        &self,
        req: Request<SetSessionReadyRequest>,
    ) -> Result<Response<SetSessionReadyResponse>, Status> {
        let SetSessionReadyRequest { machine_id } = req.into_inner();
        let machine_id = machine_id.into();
        {
            let mut sessions = self.sessions.write();
            match sessions.set_ready(machine_id) {
                Ok(SetReadyAction::RunSingle {
                    player_id,
                    difficulty,
                }) => {
                    let started_at = UtcDateTime::now();

                    let game_id = self
                        .data_source
                        .insert_game_single(machine_id, player_id, started_at, difficulty)
                        .await
                        .map_err(|e| Status::internal(e.to_string()))?;

                    if self
                        .event_tx
                        .send(Event {
                            game_id,
                            target: EventTarget::Single(machine_id),
                            data: Ok(SuccessfulEvent::GameStarted { difficulty }),
                        })
                        .is_err()
                    {
                        warn!("all receiver has been dropped");
                    };

                    let (complete_tx, complete_rx) = mpsc::channel(16);

                    tokio::spawn(game_timer(
                        EventTarget::Single(machine_id),
                        game_id,
                        self.event_tx.clone(),
                        complete_tx,
                    ));

                    // handle game completion
                    tokio::spawn(self.clone().handle_game_completion(
                        game_id,
                        EventTarget::Single(machine_id),
                        complete_rx,
                    ));
                    Ok(Response::new(SetSessionReadyResponse {}))
                }
                Ok(SetReadyAction::RunMulti {
                    player_ids,
                    difficulty,
                }) => {
                    let started_at = UtcDateTime::now();

                    let game_id = self
                        .data_source
                        .insert_game_multi(player_ids, started_at, difficulty)
                        .await
                        .map_err(|e| Status::internal(e.to_string()))?;

                    if self
                        .event_tx
                        .send(Event {
                            game_id,
                            target: EventTarget::Single(machine_id),
                            data: Ok(SuccessfulEvent::GameStarted { difficulty }),
                        })
                        .is_err()
                    {
                        warn!("all receiver has been dropped");
                    };

                    let (complete_tx, complete_rx) = mpsc::channel(16);

                    tokio::spawn(game_timer(
                        EventTarget::All,
                        game_id,
                        self.event_tx.clone(),
                        complete_tx,
                    ));

                    // handle game completion
                    tokio::spawn(self.clone().handle_game_completion(
                        game_id,
                        EventTarget::All,
                        complete_rx,
                    ));
                    Ok(Response::new(SetSessionReadyResponse {}))
                }
                Ok(SetReadyAction::WaitForAnother) => {
                    todo!()
                }
                Err(e) => {
                    return Err(Status::failed_precondition(e.to_string()));
                }
            }
        }
    }
}

/// Filters events from `event_rx` by `machine_id` and pass it through the response stream.
#[instrument(skip(event_rx, stream_tx))]
async fn pass_events_through_by_machine_id(
    machine_id: MachineId,
    event_rx: broadcast::Receiver<Event>,
    stream_tx: mpsc::Sender<Result<ListenEventsResponse, Status>>,
) {
    let events = BroadcastStream::new(event_rx);
    let mut events = events
        .map(|v| v.expect("broadcast channel lagged"))
        .filter(|ev| match ev.target {
            EventTarget::Single(ev_machine_id) => ev_machine_id == machine_id,
            EventTarget::All => true,
        });
    while let Some(ev) = events.next().await {
        let data = ev.data.map(|v| ListenEventsResponse {
            event: Some(struckout_proto::Event {
                event_data: Some(v.into()),
            }),
        });
        stream_tx.send(data).await.expect("receiver dropped");
    }
}

/// Sends event to broadcast channel every secounds until the game finishes, then sends the [`Event::GameFinished`].
#[instrument(skip(tx, complete_tx))]
async fn game_timer(
    ev_target: EventTarget,
    game_id: GameId,
    tx: broadcast::Sender<Event>,
    complete_tx: mpsc::Sender<()>,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    let duration_secs = GAME_DURATION.whole_seconds();
    for i in 0..duration_secs {
        trace!(secs_elapsed = i, "timer tick");
        let res = tx.send(Event {
            game_id,
            target: ev_target,
            data: Ok(SuccessfulEvent::GameTimeLimitNotify {
                remaining: (duration_secs - i).seconds(),
            }),
        });
        if res.is_err() {
            warn!("all event receiver has been dropped");
        }
        interval.tick().await;
    }
    complete_tx.send(()).await.expect("complete channel closed");
}

// Events which occur in the service, including errors.
#[derive(Debug, Clone)]
pub struct Event {
    game_id: GameId,
    target: EventTarget,
    data: Result<SuccessfulEvent, Status>,
}

/// Where to send events to.
#[derive(Debug, Clone, Copy)]
pub enum EventTarget {
    Single(MachineId),
    All,
}

/// Events which occur in the service. This can be converted to [`struckout_proto::Event`].
///
/// With this type, you don't need to convert proto types like `u32` or [`prost_types::Duration`]
/// to domain type like [`GameId`] or [`time::SignedDuration`] each time.
#[derive(Debug, Clone)]
pub enum SuccessfulEvent {
    /// [`struckout_proto::event::GameStarted`]
    GameStarted { difficulty: Difficulty },
    /// [`struckout_proto::event::GameTimeLimitNotify`]
    GameTimeLimitNotify { remaining: time::SignedDuration },
    /// [`struckout_proto::event::GameScoreChanged`]
    GameScoreChanged { total_score: u32 },
    /// [`struckout_proto::event::GameFinished`]
    GameFinished,
}

impl From<SuccessfulEvent> for struckout_proto::event::EventData {
    fn from(ev: SuccessfulEvent) -> Self {
        use struckout_proto::event::{
            EventData, GameFinished, GameScoreChanged, GameStarted, GameTimeLimitNotify,
        };

        match ev {
            SuccessfulEvent::GameStarted { difficulty } => EventData::GameStarted(GameStarted {
                difficulty: difficulty.into(),
            }),
            SuccessfulEvent::GameScoreChanged { total_score } => {
                EventData::GameScoreChanged(GameScoreChanged { total_score })
            }
            SuccessfulEvent::GameTimeLimitNotify { remaining } => {
                EventData::GameTimeLimitNotify(GameTimeLimitNotify {
                    remaining: Some(prost_types::Duration {
                        seconds: remaining.whole_seconds(),
                        nanos: remaining.subsec_nanoseconds(),
                    }),
                })
            }
            SuccessfulEvent::GameFinished => EventData::GameFinished(GameFinished {}),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::assert_matches;

    use struckout_proto::event::EventData;

    use crate::{AddPlayerError, PlayerId};

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
            todo!()
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

        async fn get_player(
            &self,
            _name: impl Into<String> + Send,
        ) -> Result<Player, GetPlayerError> {
            unimplemented!()
        }
    }

    /// Waits for event matching `ev_pat`. Returns when a event from stream matches pattern, or panics when stream ends.
    macro_rules! wait_event {
        ($stream_var:ident, $ev_pat:pat) => {
            loop {
                use tokio_stream::StreamExt;
                let Some(ev) = $stream_var.next().await else {
                    panic!("event stream ended");
                };
                match ev {
                    Ok(StartSessionResponse {
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

    #[tokio::test]
    async fn start_session_triggers_game_started_event() {
        let ds = StubDataSource {
            game_id: GameId::new(14),
            player_id: PlayerId::new(334),
        };
        let machine_id = MachineId::new(1);
        let difficulty = Difficulty::Normal;
        let service = GameMasterSessionServiceImpl::new(ds.clone());
        let mut rx = service.event_tx.subscribe();

        let req = Request::new(StartSessionRequest {
            machine_id: machine_id.into(),
            player_id: ds.player_id.into(),
            difficulty: difficulty.into(),
            mode: GameMode::Single.into(),
        });
        service.start_session(req).await.expect("should succeed");

        let ev = rx.recv().await.unwrap();
        let Ok(SuccessfulEvent::GameStarted {
            difficulty: ev_difficulty,
        }) = &ev.data
        else {
            panic!("should not be other events: {:?}", ev);
        };
        assert_eq!(ds.game_id, ev.game_id,);
        assert_eq!(machine_id, ev.machine_id);
        assert_eq!(difficulty, *ev_difficulty);
    }

    #[tokio::test(start_paused = true)]
    async fn start_session_stream_notifies_in_correct_order() {
        let ds = StubDataSource {
            game_id: GameId::new(20),
            player_id: PlayerId::new(13),
        };
        let machine_id = MachineId::new(2);
        let difficulty = Difficulty::Normal;
        let service = GameMasterSessionServiceImpl::new(ds.clone());

        let req = {
            let difficulty: i32 = difficulty.into();

            Request::new(StartSessionRequest {
                machine_id: machine_id.into_inner(),
                player_id: ds.player_id.into_inner(),
                difficulty,
                mode: GameMode::Single.into(),
            })
        };
        let stream = service.start_session(req).await.expect("should succeed");
        let mut stream = stream.into_inner();

        // Assert: Started
        let Some(Ok(started)) = stream.next().await else {
            panic!("stream shouldn't finish nor have error");
        };
        let Some(struckout_proto::Event {
            game_id: ev_game_id,
            event_data: Some(EventData::GameStarted(started)),
        }) = started.event
        else {
            panic!("event data didn't match: {:?}", started.event)
        };
        assert_eq!(ev_game_id, ds.game_id.into_inner());
        {
            let difficulty: i32 = difficulty.into();
            assert_eq!(started.difficulty, difficulty);
        }

        // Assert: TimeLimitNotify
        let finished = loop {
            let resp = stream.next().await.unwrap();
            let resp = resp.expect("should not have error");
            let ev = resp.event.unwrap();
            assert_eq!(ev.game_id, ds.game_id.into_inner());
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
    async fn start_session_adds_to_and_removes_from_running_games() {
        let ds = StubDataSource {
            game_id: GameId::new(20),
            player_id: PlayerId::new(13),
        };
        let machine_id = MachineId::new(2);
        let difficulty = Difficulty::Normal;
        let service = GameMasterSessionServiceImpl::new(ds.clone());

        let req = {
            let difficulty: i32 = difficulty.into();

            Request::new(StartSessionRequest {
                machine_id: machine_id.into_inner(),
                player_id: ds.player_id.into_inner(),
                difficulty,
                mode: GameMode::Single.into(),
            })
        };
        let stream = service.start_session(req).await.expect("should succeed");
        let mut stream = stream.into_inner();

        wait_event!(stream, EventData::GameStarted(_));
        {
            let guard = service.running_games.read();
            guard.get(&ds.game_id).expect("should exist");
        }

        wait_event!(stream, EventData::GameFinished(_));
        {
            let guard = service.running_games.read();
            let opt = guard.get(&ds.game_id);
            assert!(opt.is_none());
        }
    }

    #[tokio::test(start_paused = true)]
    async fn listen_events_filters_events_by_machine_id() {
        let ds = StubDataSource {
            game_id: GameId::new(14),
            player_id: PlayerId::new(334),
        };
        let machine_id_to_listen = MachineId::new(1);
        let machine_id_to_ignore = MachineId::new(2);
        let difficulty = Difficulty::Normal;
        let service = GameMasterSessionServiceImpl::new(ds.clone());

        let stream = service
            .listen_events(Request::new(ListenEventsRequest {
                machine_id: machine_id_to_listen.into_inner(),
            }))
            .await
            .expect("should succeed");
        let mut stream = stream.into_inner();

        service
            .start_session(Request::new(StartSessionRequest {
                machine_id: machine_id_to_listen.into_inner(),
                difficulty: difficulty.into(),
                player_id: ds.player_id.into_inner(),
                mode: GameMode::Single.into(),
            }))
            .await
            .unwrap();
        service
            .start_session(Request::new(StartSessionRequest {
                machine_id: machine_id_to_ignore.into_inner(),
                difficulty: difficulty.into(),
                player_id: ds.player_id.into_inner(),
                mode: GameMode::Single.into(),
            }))
            .await
            .unwrap();

        loop {
            let resp = stream.next().await.unwrap();
            let resp = resp.expect("should not have error");
            let ev = resp.event.unwrap();
            if let EventData::GameFinished(_) = ev.event_data.unwrap() {
                break;
            }
        }
    }
}
