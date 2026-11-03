mod player;
pub use player::GameMasterPlayerServiceImpl;

use std::{pin::Pin, sync::Arc, time::Duration};

use parking_lot::RwLock;
use struckout_proto::{
    self, AddScoreRequest, AddScoreResponse, Difficulty, GameMode, GetGameResultRequest,
    GetGameResultResponse, GoNextTutorialRequest, GoNextTutorialResponse,
    GoPreviousTutorialRequest, GoPreviousTutorialResponse, ListenEventsRequest,
    ListenEventsResponse, SetGameModeRequest, SetGameModeResponse, SetSessionDifficultyRequest,
    SetSessionDifficultyResponse, SetSessionReadyRequest, SetSessionReadyResponse,
    StartSessionRequest, StartSessionResponse, StartTutorialRequest, StartTutorialResponse,
    game_master_session_service_server::GameMasterSessionService,
    types::{GameId, MachineId},
};
use time::UtcDateTime;
use tokio::{
    sync::{broadcast, mpsc},
    time::Instant,
};
use tokio_stream::{Stream, wrappers::ReceiverStream};
use tonic::{Request, Response, Status};
use tracing::{instrument, warn};

use crate::{
    DataSource, DataSourceImpl, GetGameResultError,
    session::{MachineSession, ReadyConfiguration, SessionError, Sessions},
};

const GAME_DURATION: Duration = Duration::from_secs(90);

/// The session state orders database transitions and events so reconnecting clients see a consistent game.
#[derive(derive_more::Debug, Clone)]
pub struct GameMasterSessionServiceImpl<D = DataSourceImpl> {
    sessions: Arc<RwLock<Sessions>>,
    #[debug(skip)]
    data_source: D,
    event_tx: broadcast::Sender<Event>,
}

impl<D: DataSource> GameMasterSessionServiceImpl<D> {
    pub fn new(data_source: D) -> Self {
        let (event_tx, _) = broadcast::channel(32);
        Self {
            sessions: Arc::new(RwLock::new(Sessions::new())),
            data_source,
            event_tx,
        }
    }

    /// Startup owns the reservation even if the Ready RPC is cancelled while the database is responding.
    async fn start_game(self, configuration: ReadyConfiguration) -> Result<GameId, Status> {
        let game_id = match self
            .data_source
            .insert_game_single(
                configuration.machine_id,
                configuration.player_id,
                UtcDateTime::now(),
                configuration.difficulty,
            )
            .await
        {
            Ok(game_id) => game_id,
            Err(error) => {
                self.sessions.write().rollback_start(configuration);
                warn!(?error, machine_id = %configuration.machine_id, "failed to insert game");
                return Err(Status::internal("failed to create game"));
            }
        };
        let deadline = Instant::now() + GAME_DURATION;
        {
            let mut sessions = self.sessions.write();
            sessions.start(configuration, game_id, deadline);
            self.publish(
                configuration.machine_id,
                game_id,
                SuccessfulEvent::GameStarted {
                    difficulty: configuration.difficulty,
                },
            );
            self.publish(
                configuration.machine_id,
                game_id,
                SuccessfulEvent::GameTimeLimitNotify {
                    remaining: GAME_DURATION,
                },
            );
            self.publish(
                configuration.machine_id,
                game_id,
                SuccessfulEvent::GameScoreChanged { total_score: 0 },
            );
        }
        tokio::spawn(self.run_game(configuration.machine_id, game_id, deadline));
        Ok(game_id)
    }

    /// A deadline rather than a tick count prevents long frames from extending the game's scoring window.
    async fn run_game(self, machine_id: MachineId, game_id: GameId, deadline: Instant) {
        let mut next_tick = Instant::now() + Duration::from_secs(1);
        let final_score = loop {
            tokio::time::sleep_until(next_tick.min(deadline)).await;
            let now = Instant::now();
            let remaining = deadline.saturating_duration_since(now);
            let mut sessions = self.sessions.write();
            let Some(session) = sessions.get_mut(machine_id) else {
                return;
            };
            let MachineSession::Running {
                configuration,
                game_id: current_id,
                score,
                ..
            } = session
            else {
                return;
            };
            if *current_id != game_id {
                return;
            }
            self.publish(
                machine_id,
                game_id,
                SuccessfulEvent::GameTimeLimitNotify { remaining },
            );
            if remaining.is_zero() {
                let final_score = *score;
                *session = MachineSession::Finishing {
                    configuration: *configuration,
                    game_id,
                };
                break final_score;
            }
            // Do not emit a burst of old ticks after the executor was delayed.
            next_tick = now + Duration::from_secs(1);
        };
        let result = self.data_source.complete_game(game_id, final_score).await;
        {
            let mut sessions = self.sessions.write();
            sessions.remove_finished(machine_id, game_id);
            let data = match result {
                Ok(()) => Ok(SuccessfulEvent::GameFinished),
                Err(error) => {
                    warn!(?error, %game_id, %machine_id, final_score, "failed to save final game score");
                    Err(Status::internal("failed to save final game score"))
                }
            };
            // A failed save is observable as a stream error, never as a successful GameFinished.
            let _ = self.event_tx.send(Event {
                machine_id,
                game_id,
                data,
            });
        }
    }

    // Called while holding the state lock so subscription snapshots cannot be followed by older state.
    fn publish(&self, machine_id: MachineId, game_id: GameId, data: SuccessfulEvent) {
        // No subscribers is normal while the projector or touchpanel reconnects.
        let _ = self.event_tx.send(Event {
            machine_id,
            game_id,
            data: Ok(data),
        });
    }
}

fn session_status(error: SessionError) -> Status {
    match error {
        SessionError::Capacity => Status::resource_exhausted(error.to_string()),
        _ => Status::failed_precondition(error.to_string()),
    }
}

#[tonic::async_trait]
impl<D: DataSource> GameMasterSessionService for GameMasterSessionServiceImpl<D> {
    #[instrument(skip(self))]
    async fn start_session(
        &self,
        req: Request<StartSessionRequest>,
    ) -> Result<Response<StartSessionResponse>, Status> {
        let req = req.into_inner();
        self.sessions
            .write()
            .insert(req.machine_id.into(), req.player_id.into())
            .map_err(session_status)?;
        Ok(Response::new(StartSessionResponse {}))
    }

    #[instrument(skip(self))]
    async fn set_session_difficulty(
        &self,
        req: Request<SetSessionDifficultyRequest>,
    ) -> Result<Response<SetSessionDifficultyResponse>, Status> {
        let req = req.into_inner();
        let difficulty = Difficulty::try_from(req.difficulty)
            .map_err(|_| Status::invalid_argument("unknown difficulty"))?;
        if difficulty == Difficulty::Unspecified {
            return Err(Status::invalid_argument("difficulty must be specified"));
        }
        self.sessions
            .write()
            .update_difficulty(req.machine_id.into(), difficulty)
            .map_err(session_status)?;
        Ok(Response::new(SetSessionDifficultyResponse {}))
    }

    #[instrument(skip(self))]
    async fn set_game_mode(
        &self,
        req: Request<SetGameModeRequest>,
    ) -> Result<Response<SetGameModeResponse>, Status> {
        let req = req.into_inner();
        match GameMode::try_from(req.mode)
            .map_err(|_| Status::invalid_argument("unknown game mode"))?
        {
            GameMode::Single => (),
            GameMode::Multi => return Err(Status::unimplemented("multiplayer is not implemented")),
            GameMode::Unspecified => {
                return Err(Status::invalid_argument("game mode must be specified"));
            }
        }
        self.sessions
            .write()
            .set_single_mode(req.machine_id.into())
            .map_err(session_status)?;
        Ok(Response::new(SetGameModeResponse {}))
    }

    async fn start_tutorial(
        &self,
        _req: Request<StartTutorialRequest>,
    ) -> Result<Response<StartTutorialResponse>, Status> {
        Err(Status::unimplemented(
            "tutorial navigation is not implemented",
        ))
    }

    async fn go_previous_tutorial(
        &self,
        _req: Request<GoPreviousTutorialRequest>,
    ) -> Result<Response<GoPreviousTutorialResponse>, Status> {
        Err(Status::unimplemented(
            "tutorial navigation is not implemented",
        ))
    }

    async fn go_next_tutorial(
        &self,
        _req: Request<GoNextTutorialRequest>,
    ) -> Result<Response<GoNextTutorialResponse>, Status> {
        Err(Status::unimplemented(
            "tutorial navigation is not implemented",
        ))
    }

    type ListenEventsStream =
        Pin<Box<dyn Stream<Item = Result<ListenEventsResponse, Status>> + Send>>;

    #[instrument(skip(self))]
    async fn listen_events(
        &self,
        req: Request<ListenEventsRequest>,
    ) -> Result<Response<Self::ListenEventsStream>, Status> {
        let machine_id = req.into_inner().machine_id.into();
        let (receiver, initial) = {
            let sessions = self.sessions.read();
            let receiver = self.event_tx.subscribe();
            let initial = match sessions.get(machine_id) {
                Some(MachineSession::Running {
                    configuration,
                    game_id,
                    score,
                    deadline,
                }) => vec![
                    Event {
                        machine_id,
                        game_id: *game_id,
                        data: Ok(SuccessfulEvent::GameStarted {
                            difficulty: configuration.difficulty,
                        }),
                    },
                    Event {
                        machine_id,
                        game_id: *game_id,
                        data: Ok(SuccessfulEvent::GameTimeLimitNotify {
                            remaining: deadline.saturating_duration_since(Instant::now()),
                        }),
                    },
                    Event {
                        machine_id,
                        game_id: *game_id,
                        data: Ok(SuccessfulEvent::GameScoreChanged {
                            total_score: *score,
                        }),
                    },
                ],
                _ => Vec::new(),
            };
            (receiver, initial)
        };
        let (tx, rx) = mpsc::channel(32);
        tokio::spawn(pass_events_through_by_machine_id(
            machine_id, receiver, tx, initial,
        ));
        Ok(Response::new(Box::pin(ReceiverStream::new(rx))))
    }

    #[instrument(skip(self))]
    async fn add_score(
        &self,
        req: Request<AddScoreRequest>,
    ) -> Result<Response<AddScoreResponse>, Status> {
        let req = req.into_inner();
        let machine_id = req.machine_id.into();
        let game_id = req.game_id.into();
        let mut sessions = self.sessions.write();
        let Some(MachineSession::Running {
            game_id: current_id,
            score,
            deadline,
            ..
        }) = sessions.get_mut(machine_id)
        else {
            return Err(Status::failed_precondition("machine has no running game"));
        };
        if *current_id != game_id {
            return Err(Status::not_found("game does not belong to this machine"));
        }
        if Instant::now() >= *deadline {
            return Err(Status::failed_precondition("game has finished"));
        }
        *score = score
            .checked_add(req.score_to_add)
            .ok_or_else(|| Status::out_of_range("score overflow"))?;
        self.publish(
            machine_id,
            game_id,
            SuccessfulEvent::GameScoreChanged {
                total_score: *score,
            },
        );
        Ok(Response::new(AddScoreResponse {}))
    }

    async fn get_game_result(
        &self,
        req: Request<GetGameResultRequest>,
    ) -> Result<Response<GetGameResultResponse>, Status> {
        match self
            .data_source
            .get_game_result(req.into_inner().game_id.into())
            .await
        {
            Ok(record) => Ok(Response::new(GetGameResultResponse {
                total_score: record.score,
            })),
            Err(e @ GetGameResultError::GameNotFound) => Err(Status::not_found(e.to_string())),
            Err(e @ GetGameResultError::NotYetCompleted) => Err(Status::aborted(e.to_string())),
            Err(GetGameResultError::Sqlx(error)) => {
                warn!(?error, "failed to fetch game result");
                Err(Status::internal("failed to fetch game result"))
            }
        }
    }

    #[instrument(skip(self))]
    async fn set_session_ready(
        &self,
        req: Request<SetSessionReadyRequest>,
    ) -> Result<Response<SetSessionReadyResponse>, Status> {
        let configuration = self
            .sessions
            .write()
            .reserve_start(req.into_inner().machine_id.into())
            .map_err(session_status)?;
        let service = self.clone();
        let game_id = tokio::spawn(service.start_game(configuration))
            .await
            .map_err(|error| {
                warn!(?error, "game startup task failed");
                Status::internal("game startup failed")
            })??;
        Ok(Response::new(SetSessionReadyResponse {
            game_id: game_id.into_inner(),
        }))
    }
}

/// Each subscription owns a bounded forwarder and ends as soon as its response stream is dropped.
async fn pass_events_through_by_machine_id(
    machine_id: MachineId,
    mut receiver: broadcast::Receiver<Event>,
    tx: mpsc::Sender<Result<ListenEventsResponse, Status>>,
    initial: Vec<Event>,
) {
    for event in initial {
        if tx.send(event.into_response()).await.is_err() {
            return;
        }
    }
    loop {
        tokio::select! {
            _ = tx.closed() => return,
            received = receiver.recv() => match received {
                Ok(event) if event.machine_id == machine_id => {
                    if tx.send(event.into_response()).await.is_err() { return; }
                }
                Ok(_) => (),
                Err(broadcast::error::RecvError::Closed) => return,
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    let _ = tx.send(Err(Status::resource_exhausted("event stream lagged; reconnect to resynchronize"))).await;
                    return;
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
struct Event {
    game_id: GameId,
    machine_id: MachineId,
    data: Result<SuccessfulEvent, Status>,
}

#[derive(Debug, Clone)]
enum SuccessfulEvent {
    GameStarted { difficulty: Difficulty },
    GameTimeLimitNotify { remaining: Duration },
    GameScoreChanged { total_score: u32 },
    GameFinished,
}

impl Event {
    fn into_response(self) -> Result<ListenEventsResponse, Status> {
        use struckout_proto::event::{
            EventData, GameFinished, GameScoreChanged, GameStarted, GameTimeLimitNotify,
        };
        let game_id = self.game_id.into_inner();
        let event_data = match self.data? {
            SuccessfulEvent::GameStarted { difficulty } => EventData::GameStarted(GameStarted {
                game_id,
                difficulty: difficulty.into(),
            }),
            SuccessfulEvent::GameTimeLimitNotify { remaining } => {
                EventData::GameTimeLimitNotify(GameTimeLimitNotify {
                    game_id,
                    remaining: Some(prost_types::Duration {
                        seconds: remaining.as_secs() as i64,
                        nanos: remaining.subsec_nanos() as i32,
                    }),
                })
            }
            SuccessfulEvent::GameScoreChanged { total_score } => {
                EventData::GameScoreChanged(GameScoreChanged {
                    game_id,
                    total_score,
                })
            }
            SuccessfulEvent::GameFinished => EventData::GameFinished(GameFinished { game_id }),
        };
        Ok(ListenEventsResponse {
            event: Some(struckout_proto::Event {
                event_data: Some(event_data),
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AddPlayerError, GetPlayerError, ValidatePlayerNameError,
        data::{GameRecord, Player},
    };
    use std::sync::atomic::{AtomicU32, Ordering};
    use struckout_proto::{event::EventData, types::PlayerId};
    use tokio_stream::StreamExt;

    #[derive(Clone)]
    struct StubDataSource {
        game_id: GameId,
        player_id: PlayerId,
        inserted: Arc<AtomicU32>,
        completed_score: Arc<AtomicU32>,
    }

    impl StubDataSource {
        fn new(game_id: u32, player_id: u32) -> Self {
            Self {
                game_id: game_id.into(),
                player_id: player_id.into(),
                inserted: Arc::new(AtomicU32::new(0)),
                completed_score: Arc::new(AtomicU32::new(0)),
            }
        }
    }

    impl DataSource for StubDataSource {
        async fn insert_game_single(
            &self,
            _machine_id: MachineId,
            _player_id: PlayerId,
            _started_at: UtcDateTime,
            _difficulty: Difficulty,
        ) -> Result<GameId, sqlx::Error> {
            self.inserted.fetch_add(1, Ordering::SeqCst);
            Ok(self.game_id)
        }
        async fn insert_game_multi(
            &self,
            _player_ids: [PlayerId; 2],
            _started_at: UtcDateTime,
            _difficulty: Difficulty,
        ) -> Result<GameId, sqlx::Error> {
            Err(sqlx::Error::Protocol("multiplayer is not supported".into()))
        }
        async fn complete_game(&self, _game_id: GameId, score: u32) -> Result<(), sqlx::Error> {
            self.completed_score.store(score, Ordering::SeqCst);
            Ok(())
        }
        async fn add_player(&self, _name: impl Into<String>) -> Result<PlayerId, AddPlayerError> {
            Ok(self.player_id)
        }
        async fn get_game_result(
            &self,
            _game_id: GameId,
        ) -> Result<GameRecord, GetGameResultError> {
            Err(GetGameResultError::GameNotFound)
        }
        async fn validate_player_name(
            &self,
            _name: impl Into<String>,
        ) -> Result<(), ValidatePlayerNameError> {
            Ok(())
        }
        async fn get_player(
            &self,
            name: impl Into<String> + Send,
        ) -> Result<Player, GetPlayerError> {
            Ok(Player {
                player_id: self.player_id,
                name: name.into(),
            })
        }
    }

    async fn configure(
        service: &GameMasterSessionServiceImpl<StubDataSource>,
        machine: u32,
        player: u32,
    ) {
        service
            .start_session(Request::new(StartSessionRequest {
                machine_id: machine,
                player_id: player,
            }))
            .await
            .unwrap();
        service
            .set_game_mode(Request::new(SetGameModeRequest {
                machine_id: machine,
                mode: GameMode::Single.into(),
            }))
            .await
            .unwrap();
        service
            .set_session_difficulty(Request::new(SetSessionDifficultyRequest {
                machine_id: machine,
                difficulty: Difficulty::Normal.into(),
            }))
            .await
            .unwrap();
    }

    async fn next_event(
        stream: &mut <GameMasterSessionServiceImpl<StubDataSource> as GameMasterSessionService>::ListenEventsStream,
    ) -> EventData {
        stream
            .next()
            .await
            .unwrap()
            .unwrap()
            .event
            .unwrap()
            .event_data
            .unwrap()
    }

    #[tokio::test]
    async fn start_session_triggers_game_started_event() {
        let ds = StubDataSource::new(14, 334);
        let service = GameMasterSessionServiceImpl::new(ds.clone());
        let mut stream = service
            .listen_events(Request::new(ListenEventsRequest { machine_id: 1 }))
            .await
            .unwrap()
            .into_inner();
        configure(&service, 1, 334).await;
        let (first, second) = tokio::join!(
            service.set_session_ready(Request::new(SetSessionReadyRequest { machine_id: 1 })),
            service.set_session_ready(Request::new(SetSessionReadyRequest { machine_id: 1 })),
        );
        assert_eq!(first.unwrap().into_inner().game_id, 14);
        assert_eq!(second.unwrap_err().code(), tonic::Code::FailedPrecondition);
        assert_eq!(ds.inserted.load(Ordering::SeqCst), 1);
        assert!(
            matches!(next_event(&mut stream).await, EventData::GameStarted(started) if started.game_id == 14 && started.difficulty == Difficulty::Normal as i32)
        );
        assert_eq!(
            service
                .set_game_mode(Request::new(SetGameModeRequest {
                    machine_id: 1,
                    mode: GameMode::Multi.into()
                }))
                .await
                .unwrap_err()
                .code(),
            tonic::Code::Unimplemented
        );
        assert_eq!(
            service
                .start_tutorial(Request::new(StartTutorialRequest { machine_id: 1 }))
                .await
                .unwrap_err()
                .code(),
            tonic::Code::Unimplemented
        );
    }

    #[tokio::test(start_paused = true)]
    async fn start_session_stream_notifies_in_correct_order() {
        let ds = StubDataSource::new(20, 13);
        let service = GameMasterSessionServiceImpl::new(ds.clone());
        let mut stream = service
            .listen_events(Request::new(ListenEventsRequest { machine_id: 2 }))
            .await
            .unwrap()
            .into_inner();
        configure(&service, 2, 13).await;
        service
            .set_session_ready(Request::new(SetSessionReadyRequest { machine_id: 2 }))
            .await
            .unwrap();
        assert!(matches!(
            next_event(&mut stream).await,
            EventData::GameStarted(_)
        ));
        assert!(
            matches!(next_event(&mut stream).await, EventData::GameTimeLimitNotify(notify) if notify.remaining.unwrap().seconds == 90)
        );
        assert!(
            matches!(next_event(&mut stream).await, EventData::GameScoreChanged(score) if score.total_score == 0)
        );
        service
            .add_score(Request::new(AddScoreRequest {
                machine_id: 2,
                game_id: 20,
                score_to_add: 7,
            }))
            .await
            .unwrap();
        assert!(
            matches!(next_event(&mut stream).await, EventData::GameScoreChanged(score) if score.total_score == 7)
        );
        assert_eq!(
            service
                .add_score(Request::new(AddScoreRequest {
                    machine_id: 1,
                    game_id: 20,
                    score_to_add: 1
                }))
                .await
                .unwrap_err()
                .code(),
            tonic::Code::FailedPrecondition
        );
        assert_eq!(
            service
                .add_score(Request::new(AddScoreRequest {
                    machine_id: 2,
                    game_id: 99,
                    score_to_add: 1
                }))
                .await
                .unwrap_err()
                .code(),
            tonic::Code::NotFound
        );
        assert_eq!(
            service
                .add_score(Request::new(AddScoreRequest {
                    machine_id: 2,
                    game_id: 20,
                    score_to_add: u32::MAX
                }))
                .await
                .unwrap_err()
                .code(),
            tonic::Code::OutOfRange
        );
        let mut snapshot = service
            .listen_events(Request::new(ListenEventsRequest { machine_id: 2 }))
            .await
            .unwrap()
            .into_inner();
        assert!(
            matches!(next_event(&mut snapshot).await, EventData::GameStarted(started) if started.game_id == 20)
        );
        assert!(matches!(
            next_event(&mut snapshot).await,
            EventData::GameTimeLimitNotify(_)
        ));
        assert!(
            matches!(next_event(&mut snapshot).await, EventData::GameScoreChanged(score) if score.total_score == 7)
        );
        let mut remaining = 90;
        loop {
            match next_event(&mut stream).await {
                EventData::GameTimeLimitNotify(notify) => {
                    let value = notify.remaining.unwrap().seconds;
                    assert!(value <= remaining);
                    remaining = value;
                }
                EventData::GameFinished(finished) => {
                    assert_eq!(finished.game_id, 20);
                    break;
                }
                other => panic!("unexpected event: {other:?}"),
            }
        }
        assert_eq!(remaining, 0);
        assert_eq!(ds.completed_score.load(Ordering::SeqCst), 7);
    }

    #[tokio::test(start_paused = true)]
    async fn start_session_adds_to_and_removes_from_running_games() {
        let service = GameMasterSessionServiceImpl::new(StubDataSource::new(20, 13));
        configure(&service, 2, 13).await;
        service
            .set_session_ready(Request::new(SetSessionReadyRequest { machine_id: 2 }))
            .await
            .unwrap();
        assert!(matches!(
            service.sessions.read().get(2.into()),
            Some(MachineSession::Running { .. })
        ));
        tokio::time::advance(GAME_DURATION).await;
        // Completion includes a database future, so yield until its detached timer finishes.
        for _ in 0..10 {
            tokio::task::yield_now().await;
        }
        assert!(service.sessions.read().get(2.into()).is_none());
        configure(&service, 2, 13).await;
        assert!(
            service
                .set_session_ready(Request::new(SetSessionReadyRequest { machine_id: 2 }))
                .await
                .is_ok()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn listen_events_filters_events_by_machine_id() {
        let service = GameMasterSessionServiceImpl::new(StubDataSource::new(14, 334));
        let mut stream = service
            .listen_events(Request::new(ListenEventsRequest { machine_id: 1 }))
            .await
            .unwrap()
            .into_inner();
        configure(&service, 2, 334).await;
        service
            .set_session_ready(Request::new(SetSessionReadyRequest { machine_id: 2 }))
            .await
            .unwrap();
        configure(&service, 1, 334).await;
        service
            .set_session_ready(Request::new(SetSessionReadyRequest { machine_id: 1 }))
            .await
            .unwrap();
        loop {
            match next_event(&mut stream).await {
                EventData::GameStarted(ev) => assert_eq!(ev.game_id, 14),
                EventData::GameFinished(ev) => {
                    assert_eq!(ev.game_id, 14);
                    break;
                }
                _ => (),
            }
        }
        drop(stream);
        tokio::task::yield_now().await;
        assert_eq!(service.event_tx.receiver_count(), 0);
    }
}
