use std::{collections::HashMap, pin::Pin, sync::Arc, time::Duration};

use parking_lot::RwLock;
use struckout_proto::{
    self, AddPlayerRequest, AddPlayerResponse, AddScoreRequest, AddScoreResponse, Difficulty,
    ListenEventsRequest, ListenEventsResponse, StartGameRequest, StartGameResponse,
    game_master_service_server::GameMasterService,
};
use time::{SignedDuration, UtcDateTime, ext::NumericalDuration};
use tokio::sync::{broadcast, mpsc};
use tokio_stream::{
    Stream, StreamExt,
    wrappers::{BroadcastStream, ReceiverStream},
};
use tonic::{Request, Response, Status};
use tracing::{instrument, trace, warn};

use crate::{AddPlayerError, DataSource, GameId, MachineId};

const GAME_DURATION: SignedDuration = SignedDuration::seconds(150);

#[derive(Debug)]
pub struct Game {
    game_id: GameId,
    score: u32,
}

impl Game {
    pub fn new(game_id: GameId) -> Self {
        Self { game_id, score: 0 }
    }
}

// Events which occur in the service, including errors.
#[derive(Debug, Clone)]
pub struct Event {
    game_id: GameId,
    machine_id: MachineId,
    data: Result<SuccessfulEvent, Status>,
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
    /// [`struckout_proto::event::GameFinished`]
    GameFinished,
}

impl From<SuccessfulEvent> for struckout_proto::event::EventData {
    fn from(ev: SuccessfulEvent) -> Self {
        use struckout_proto::event::{EventData, GameFinished, GameStarted, GameTimeLimitNotify};

        match ev {
            SuccessfulEvent::GameStarted { difficulty } => EventData::GameStarted(GameStarted {
                difficulty: difficulty.into(),
            }),

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

/// Implementation of [`GameMasterService`].
#[derive(Clone)]
pub struct GameMasterServiceImpl<D> {
    running_games: Arc<RwLock<HashMap<GameId, Game>>>,
    data_source: D,
    event_tx: broadcast::Sender<Event>,
}

impl<D> GameMasterServiceImpl<D>
where
    D: DataSource,
{
    pub fn new(data_source: D) -> Self {
        let (event_tx, _) = broadcast::channel(32);
        Self {
            running_games: Default::default(),
            data_source,
            event_tx,
        }
    }

    /// Handles completion of game when notified via `complete_rx`.
    async fn handle_game_completion(
        self,
        game_id: GameId,
        machine_id: MachineId,
        mut complete_rx: mpsc::Receiver<()>,
    ) {
        complete_rx.recv().await.expect("complete channel closed");
        let score = {
            let guard = self.running_games.read();
            let game = guard.get(&game_id).unwrap(); // guaranteed to exist
            game.score
        };
        let res = self.data_source.complete_game(game_id, score).await;
        if let Err(e) = res {
            self.event_tx
                .send(Event {
                    game_id,
                    machine_id,
                    data: Err(Status::unknown(e.to_string())), // TODO: more detailed error
                })
                .expect("all event receiver has been dropped");
        };
        {
            let mut guard = self.running_games.write();
            guard.remove(&game_id);
        }
        self.event_tx
            .send(Event {
                game_id,
                machine_id,
                data: Ok(SuccessfulEvent::GameFinished),
            })
            .expect("all event receiver has been dropped");
    }
}

#[tonic::async_trait]
impl<D> GameMasterService for GameMasterServiceImpl<D>
where
    D: DataSource,
{
    type StartGameStream = Pin<Box<dyn Stream<Item = Result<StartGameResponse, Status>> + Send>>;

    type ListenEventsStream =
        Pin<Box<dyn Stream<Item = Result<ListenEventsResponse, Status>> + Send>>;

    async fn start_game(
        &self,
        req: Request<StartGameRequest>,
    ) -> Result<Response<Self::StartGameStream>, Status> {
        trace!(
            remote_addr = ?req.remote_addr(),
            "received start_game request"
        );
        let req = req.into_inner();
        let machine_id = req.machine_id.into();
        let started_at = UtcDateTime::now();
        let difficulty: Difficulty = req
            .difficulty
            .try_into()
            .map_err(|e: prost::UnknownEnumValue| Status::invalid_argument(e.to_string()))?;

        // TODO: more detailed error
        let game_id = self
            .data_source
            .insert_game(machine_id, req.player_id.into(), started_at, difficulty)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        self.running_games
            .write()
            .insert(game_id, Game::new(game_id));

        // subscribe on events before sending `GameStarted`.
        let event_rx = self.event_tx.subscribe();
        if self
            .event_tx
            .send(Event {
                game_id,
                machine_id,
                data: Ok(SuccessfulEvent::GameStarted { difficulty }),
            })
            .is_err()
        {
            warn!("all receiver has been dropped");
        };

        // channel to send gRPC stream.
        let (stream_tx, stream_rx) = mpsc::channel(32);
        let (complete_tx, complete_rx) = mpsc::channel(16);

        tokio::spawn(game_timer(
            machine_id,
            game_id,
            self.event_tx.clone(),
            complete_tx,
        ));
        // pass events through stream
        tokio::spawn(pass_events_through_by_game_id(game_id, event_rx, stream_tx));
        // handle game completion
        tokio::spawn(
            self.clone()
                .handle_game_completion(game_id, machine_id, complete_rx),
        );

        let out_stream = ReceiverStream::new(stream_rx);
        Ok(Response::new(Box::pin(out_stream) as Self::StartGameStream))
    }

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
        {
            let mut guard = self.running_games.write();
            let Some(game) = guard.get_mut(&game_id) else {
                return Err(Status::not_found(format!(
                    "game with id {} does not exist or is not running",
                    game_id.into_inner()
                )));
            };
            game.score += req.score_to_add;
        }
        Ok(Response::new(AddScoreResponse {}))
    }

    async fn add_player(
        &self,
        req: Request<AddPlayerRequest>,
    ) -> Result<Response<AddPlayerResponse>, Status> {
        let req = req.into_inner();
        match self.data_source.add_player(req.name).await {
            Ok(player_id) => Ok(Response::new(AddPlayerResponse {
                player_id: player_id.into_inner(),
            })),
            Err(e @ AddPlayerError::NameAlreadyUsed) => Err(Status::already_exists(e.to_string())),
            Err(AddPlayerError::Sqlx(e)) => Err(Status::unavailable(e.to_string())),
        }
    }
}

/// Filters events from `event_rx` by `game_id` and pass it through the response stream.
#[instrument(skip(event_rx, stream_tx))]
async fn pass_events_through_by_game_id(
    game_id: GameId,
    event_rx: broadcast::Receiver<Event>,
    stream_tx: mpsc::Sender<Result<StartGameResponse, Status>>,
) {
    let events = BroadcastStream::new(event_rx);
    let mut events = events
        .map(|v| v.expect("broadcast channel lagged"))
        .filter(|ev| ev.game_id == game_id);
    while let Some(ev) = events.next().await {
        let data = ev.data.map(|v| StartGameResponse {
            event: Some(struckout_proto::Event {
                machine_id: ev.machine_id.into_inner(),
                game_id: ev.game_id.into_inner(),
                event_data: Some(v.into()),
            }),
        });
        stream_tx.send(data).await.expect("receiver dropped");
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
        .filter(|ev| ev.machine_id == machine_id);
    while let Some(ev) = events.next().await {
        let data = ev.data.map(|v| ListenEventsResponse {
            event: Some(struckout_proto::Event {
                machine_id: ev.machine_id.into_inner(),
                game_id: ev.game_id.into_inner(),
                event_data: Some(v.into()),
            }),
        });
        stream_tx.send(data).await.expect("receiver dropped");
    }
}

/// Sends event to broadcast channel every secounds until the game finishes, then sends the [`Event::GameFinished`].
#[instrument(skip(tx, complete_tx))]
async fn game_timer(
    machine_id: MachineId,
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
            machine_id,
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

#[cfg(test)]
mod tests {
    use std::assert_matches;

    use tracing::Level;

    use crate::{AddPlayerError, PlayerId, proto::event::EventData};

    use super::*;

    #[derive(Clone)]
    struct StubDataSource {
        game_id: GameId,
        player_id: PlayerId,
    }

    impl DataSource for StubDataSource {
        async fn insert_game(
            &self,
            _machine_id: MachineId,
            _player_id: PlayerId,
            _started_at: UtcDateTime,
            _difficulty: Difficulty,
        ) -> Result<GameId, sqlx::Error> {
            Ok(self.game_id)
        }

        async fn complete_game(&self, _game_id: GameId, _score: u32) -> Result<(), sqlx::Error> {
            Ok(())
        }

        async fn add_player(&self, _name: impl Into<String>) -> Result<PlayerId, AddPlayerError> {
            Ok(self.player_id)
        }
    }

    #[tokio::test]
    async fn start_game_triggers_game_started_event() {
        let ds = StubDataSource {
            game_id: GameId(14),
            player_id: PlayerId(334),
        };
        let machine_id = MachineId(1);
        let difficulty = Difficulty::Normal;
        let service = GameMasterServiceImpl::new(ds.clone());
        let mut rx = service.event_tx.subscribe();

        let req = Request::new(StartGameRequest {
            machine_id: machine_id.into(),
            player_id: ds.player_id.into(),
            difficulty: difficulty.into(),
        });
        service.start_game(req).await.expect("should succeed");

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
    async fn start_game_stream_notifies_in_correct_order() {
        tracing::subscriber::set_global_default(
            tracing_subscriber::FmtSubscriber::builder()
                .with_max_level(Level::TRACE)
                .finish(),
        )
        .expect("failed to set default subscriber");

        let ds = StubDataSource {
            game_id: GameId(20),
            player_id: PlayerId(13),
        };
        let machine_id = MachineId(2);
        let difficulty = Difficulty::Normal;
        let service = GameMasterServiceImpl::new(ds.clone());

        let req = {
            let difficulty: i32 = difficulty.into();

            Request::new(StartGameRequest {
                machine_id: machine_id.into_inner(),
                player_id: ds.player_id.into_inner(),
                difficulty,
            })
        };
        let stream = service.start_game(req).await.expect("should succeed");
        let mut stream = stream.into_inner();

        // Assert: Started
        let Some(Ok(started)) = stream.next().await else {
            panic!("stream shouldn't finish nor have error");
        };
        let Some(proto::Event {
            game_id: ev_game_id,
            machine_id: ev_machine_id,
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
        assert_eq!(ev_machine_id, machine_id.into_inner());

        // Assert: TimeLimitNotify
        let finished = loop {
            let resp = stream.next().await.unwrap();
            let resp = resp.expect("should not have error");
            let ev = resp.event.unwrap();
            assert_eq!(ev.machine_id, machine_id.into_inner());
            assert_eq!(ev.game_id, ds.game_id.into_inner());
            let ev = ev.event_data.unwrap();
            let EventData::GameTimeLimitNotify(notify) = ev else {
                break ev;
            };
            let rem = notify.remaining.unwrap();
        };

        // Assert: Finished
        assert_matches!(finished, EventData::GameFinished(_));
    }

    #[tokio::test]
    async fn start_game_adds_to_and_removes_from_running_games() {
        todo!()
    }

    #[tokio::test(start_paused = true)]
    async fn listen_events_filters_events_by_machine_id() {
        let ds = StubDataSource {
            game_id: GameId(14),
            player_id: PlayerId(334),
        };
        let machine_id_to_listen = MachineId(1);
        let machine_id_to_ignore = MachineId(2);
        let difficulty = Difficulty::Normal;
        let service = GameMasterServiceImpl::new(ds.clone());

        let stream = service
            .listen_events(Request::new(ListenEventsRequest {
                machine_id: machine_id_to_listen.into_inner(),
            }))
            .await
            .expect("should succeed");
        let mut stream = stream.into_inner();

        service
            .start_game(Request::new(StartGameRequest {
                machine_id: machine_id_to_listen.into_inner(),
                difficulty: difficulty.into(),
                player_id: ds.player_id.into_inner(),
            }))
            .await
            .unwrap();
        service
            .start_game(Request::new(StartGameRequest {
                machine_id: machine_id_to_ignore.into_inner(),
                difficulty: difficulty.into(),
                player_id: ds.player_id.into_inner(),
            }))
            .await
            .unwrap();

        loop {
            let resp = stream.next().await.unwrap();
            let resp = resp.expect("should not have error");
            let ev = resp.event.unwrap();
            assert_eq!(ev.machine_id, machine_id_to_listen.into_inner());
            if let EventData::GameFinished(_) = ev.event_data.unwrap() {
                break;
            }
        }
    }
}
