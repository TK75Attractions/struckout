use std::{collections::HashMap, pin::Pin, sync::Arc, time::Duration};

use parking_lot::RwLock;
use sqlx::{MySql, Pool};
use time::{SignedDuration, UtcDateTime, ext::NumericalDuration};
use tokio::sync::{broadcast, mpsc};
use tokio_stream::{
    Stream, StreamExt,
    wrappers::{BroadcastStream, ReceiverStream},
};
use tonic::{Request, Response, Status};
use tracing::{trace, warn};

use crate::{
    DataSource, GameId, MachineId, PlayerId, data,
    proto::{
        self, AddPlayerRequest, AddPlayerResponse, AddScoreRequest, AddScoreResponse, Difficulty,
        ListenEventsRequest, StartGameRequest, StartGameResponse,
        event::{EventData, GameTimeLimitNotify},
        game_master_service_server::GameMasterService,
        start_game_response::StartGameRespData,
    },
};

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

/// Events which occur in the service. This can be converted to [`crate::proto::Event`].
///
/// With this type, you don't need to convert proto types like `u32` or [`prost_types::Duration`]
/// to domain type like [`GameId`] or [`time::SignedDuration`] each time.
#[derive(Debug, Clone)]
pub enum Event {
    /// [`crate::proto::event::GameStarted`]
    GameStarted {
        game_id: GameId,
        machine_id: MachineId,
        difficulty: Difficulty,
    },
    /// [`crate::proto::event::GameTimeLimitNotify`]
    GameTimeLimitNotify {
        game_id: GameId,
        machine_id: MachineId,
        remaining: time::SignedDuration,
    },
    /// [`crate::proto::event::GameFinished`]
    GameFinished {
        game_id: GameId,
        machine_id: MachineId,
    },
}

impl Event {
    /// Returns the game id of event.
    pub fn game_id(&self) -> GameId {
        match self {
            Event::GameStarted { game_id, .. } => *game_id,
            Event::GameTimeLimitNotify { game_id, .. } => *game_id,
            Event::GameFinished { game_id, .. } => *game_id,
        }
    }

    /// Returns the machine id of event.
    pub fn machine_id(&self) -> MachineId {
        match self {
            Event::GameStarted { machine_id, .. } => *machine_id,
            Event::GameTimeLimitNotify { machine_id, .. } => *machine_id,
            Event::GameFinished { machine_id, .. } => *machine_id,
        }
    }
}

impl From<Event> for proto::Event {
    fn from(ev: Event) -> Self {
        use proto::event::{EventData, GameFinished, GameStarted, GameTimeLimitNotify};

        match ev {
            Event::GameStarted {
                game_id,
                machine_id,
                difficulty,
            } => proto::Event {
                event_data: Some(EventData::GameStarted(GameStarted {
                    machine_id: machine_id.into(),
                    difficulty: difficulty.into(),
                    game_id: game_id.into(),
                })),
            },
            Event::GameTimeLimitNotify {
                game_id,
                machine_id,
                remaining,
            } => proto::Event {
                event_data: Some(EventData::GameTimeLimitNotify(GameTimeLimitNotify {
                    game_id: game_id.into(),
                    machine_id: machine_id.into(),
                    remaining: Some(prost_types::Duration {
                        seconds: remaining.whole_seconds(),
                        nanos: remaining.subsec_nanoseconds(),
                    }),
                })),
            },
            Event::GameFinished {
                game_id,
                machine_id,
            } => proto::Event {
                event_data: Some(EventData::GameFinished(GameFinished {
                    game_id: game_id.into(),
                    machine_id: machine_id.into(),
                })),
            },
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

impl<D> GameMasterServiceImpl<D> {
    pub fn new(data_source: D) -> Self {
        let (event_tx, _) = broadcast::channel(32);
        Self {
            running_games: Default::default(),
            data_source,
            event_tx,
        }
    }
}

#[tonic::async_trait]
impl<D> GameMasterService for GameMasterServiceImpl<D>
where
    D: DataSource,
{
    type StartGameStream = Pin<Box<dyn Stream<Item = Result<StartGameResponse, Status>> + Send>>;

    type ListenEventsStream = Pin<Box<dyn Stream<Item = Result<proto::Event, Status>> + Send>>;

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
        if let Err(_) = self.event_tx.send(Event::GameStarted {
            game_id,
            machine_id,
            difficulty,
        }) {
            warn!("all receiver has been dropped");
        };

        // channel to send gRPC stream.
        let (tx, rx) = mpsc::channel(32);
        tokio::spawn(game_timer(machine_id, game_id, self.event_tx.clone()));

        let event_rx = self.event_tx.subscribe();
        tokio::spawn(async move {
            let events = BroadcastStream::new(event_rx);
            let mut events = events
                .map(|v| v.expect("broadcast channel lagged"))
                .filter(|ev| ev.game_id() == game_id);
            while let Some(ev) = events.next().await {
                tx.send(Ok(StartGameResponse {
                    start_game_resp_data: Some(StartGameRespData::Event(ev.into())),
                }))
                .await
                .expect("receiver dropped");
            }
        });

        let out_stream = ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(out_stream) as Self::StartGameStream))
    }

    async fn listen_events(
        &self,
        req: Request<ListenEventsRequest>,
    ) -> Result<Response<Self::ListenEventsStream>, Status> {
        todo!()
    }

    async fn add_score(
        &self,
        req: Request<AddScoreRequest>,
    ) -> Result<Response<AddScoreResponse>, Status> {
        todo!()
    }

    async fn add_player(
        &self,
        req: Request<AddPlayerRequest>,
    ) -> Result<Response<AddPlayerResponse>, Status> {
        todo!()
    }
}

/// Sends event to broadcast every secounds until the game finishes.
async fn game_timer(machine_id: MachineId, game_id: GameId, tx: broadcast::Sender<Event>) {
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    let duration_secs = GAME_DURATION.whole_seconds();
    for i in 0..duration_secs {
        let res = tx.send(Event::GameTimeLimitNotify {
            game_id,
            machine_id,
            remaining: (duration_secs - i).seconds(),
        });
        if res.is_err() {
            warn!("all event receiver has been dropped");
        }
        interval.tick().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct StubDataSource {
        game_id: GameId,
        player_id: PlayerId,
    }

    impl DataSource for StubDataSource {
        async fn insert_game(
            &self,
            machine_id: MachineId,
            player_id: PlayerId,
            started_at: UtcDateTime,
            difficulty: Difficulty,
        ) -> Result<GameId, sqlx::Error> {
            Ok(self.game_id)
        }

        async fn complete_game(&self, game_id: GameId, score: u32) -> Result<(), sqlx::Error> {
            unimplemented!()
        }

        async fn add_player(&self, name: impl Into<String>) -> Result<PlayerId, sqlx::Error> {
            Ok(self.player_id)
        }
    }

    #[tokio::test]
    async fn start_game_triggers_game_started_event() {
        let ds = StubDataSource {
            game_id: GameId(1),
            player_id: PlayerId(1),
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
        let Event::GameStarted {
            game_id: ev_game_id,
            machine_id: ev_machine_id,
            difficulty: ev_difficulty,
        } = &ev
        else {
            panic!("should not be other events: {:?}", ev);
        };
        assert_eq!(ds.game_id, *ev_game_id,);
        assert_eq!(machine_id, *ev_machine_id);
        assert_eq!(difficulty, (*ev_difficulty).into());
    }

    #[tokio::test]
    async fn start_game_stream_notifies_every_secounds() {
        todo!()
    }
}
