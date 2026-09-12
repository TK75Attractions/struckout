use std::sync::Arc;

use derive_getters::Getters;
use futures_util::Stream;
use parking_lot::RwLock;
use struckout_proto::{
    AddPlayerRequest, AddPlayerResponse, Difficulty, StartGameRequest, StartGameResponse,
    event::EventData, game_master_service_client::GameMasterServiceClient,
};
use thiserror::Error;
use tokio::sync::watch;
use tokio_stream::StreamExt;
use tonic::{Response, Status, transport::Endpoint};

use crate::data::{MachineId, PlayerId, remaining_time::DisplayableRemainingTime};

const GAME_MASTER_GRPC_PORT: &str = env!("TOUCHPANEL_GAME_MASTER_GRPC_PORT");

/// A trait for [`GameMasterServiceClient`].
pub trait GameMasterGrpcClient: Sized + Sync + Send + Clone {
    fn connect<D>(dst: D) -> impl Future<Output = Result<Self, tonic::transport::Error>>
    where
        D: TryInto<tonic::transport::Endpoint>,
        D::Error: Into<Box<dyn std::error::Error + Send + Sync + 'static>>;

    fn add_player(
        &mut self,
        request: impl tonic::IntoRequest<AddPlayerRequest>,
    ) -> impl Future<Output = Result<Response<AddPlayerResponse>, Status>>;

    fn start_game(
        &mut self,
        request: impl tonic::IntoRequest<StartGameRequest>,
    ) -> impl Future<
        Output = Result<
            Response<
                impl Stream<Item = Result<StartGameResponse, Status>> + Unpin + Send + Sync + 'static,
            >,
            Status,
        >,
    >;
}

#[derive(derive_more::Debug, Clone)]
pub struct GameMasterClient<T: GameMasterGrpcClient> {
    machine_id: MachineId,
    client: T,
    session: Arc<RwLock<Option<Session>>>,
}

/// Error returned from [`GameMasterClient::connect()`].
#[derive(Debug, Error)]
pub enum ConnectError {
    #[error("given server address {server_addr} is invalid: {source}")]
    InvalidServerAddr {
        server_addr: String,
        source: tonic::transport::Error,
    },
    #[error(transparent)]
    Other(#[from] tonic::transport::Error),
}

/// Returned from [`GameMasterClient`]'s method which sends request to server.
#[derive(Debug, Error)]
pub enum RequestError {
    #[error(transparent)]
    Grpc(#[from] Status),
    #[error("stream unexpectedly ended")]
    UnexpectedEndOfStream,
    #[error("field {field_name} of message {message_name} was invalid: {detail}")]
    InvalidPacket {
        message_name: String,
        field_name: String,
        detail: String,
    },
    #[error("unexpected event was sent from server: {0:?}")]
    UnexpectedEvent(struckout_proto::Event),
}

impl RequestError {
    /// Utility method to construct [`RequestError::InvalidPacket`].
    fn missing_field(message_name: &str, field_name: &str) -> Self {
        Self::InvalidPacket {
            message_name: message_name.to_string(),
            field_name: field_name.to_string(),
            detail: "field was missing despite it should include some data".to_string(),
        }
    }

    /// Utility method to construct [`RequestError::InvalidPacket`].
    fn invalid_packet(message_name: &str, field_name: &str, detail: &str) -> Self {
        Self::InvalidPacket {
            message_name: message_name.to_string(),
            field_name: field_name.to_string(),
            detail: detail.to_string(),
        }
    }
}

impl GameMasterGrpcClient for GameMasterServiceClient<tonic::transport::Channel> {
    fn connect<D>(dst: D) -> impl Future<Output = Result<Self, tonic::transport::Error>>
    where
        D: TryInto<tonic::transport::Endpoint>,
        D::Error: Into<Box<dyn std::error::Error + Send + Sync + 'static>>,
    {
        Self::connect(dst)
    }

    fn add_player(
        &mut self,
        request: impl tonic::IntoRequest<AddPlayerRequest>,
    ) -> impl Future<Output = Result<Response<AddPlayerResponse>, Status>> {
        self.add_player(request)
    }

    fn start_game(
        &mut self,
        request: impl tonic::IntoRequest<StartGameRequest>,
    ) -> impl Future<
        Output = Result<
            Response<
                impl Stream<Item = Result<StartGameResponse, Status>> + Unpin + Send + Sync + 'static,
            >,
            Status,
        >,
    > {
        self.start_game(request)
    }
}

impl<T: GameMasterGrpcClient> GameMasterClient<T> {
    pub async fn connect(server_addr: &str, machine_id: MachineId) -> Result<Self, ConnectError> {
        let endpoint = format!("{}:{}", server_addr, GAME_MASTER_GRPC_PORT);
        let endpoint = Endpoint::new(endpoint).map_err(|e| ConnectError::InvalidServerAddr {
            server_addr: server_addr.to_string(),
            source: e,
        })?;

        let client = T::connect(endpoint).await?;
        Ok(Self {
            machine_id,
            client,
            session: Arc::new(RwLock::new(None)),
        })
    }

    pub async fn add_player(&mut self, name: impl Into<String>) -> Result<PlayerId, Status> {
        self.client
            .add_player(AddPlayerRequest { name: name.into() })
            .await
            .map(|res| res.into_inner().player_id.into())
    }

    /// Starts a new game.
    ///
    /// It returns when the first response from the server came.
    /// Subsequent responses (e.g. ScoreChanged) are handled internally in another task.
    pub async fn start_game(
        &mut self,
        player_id: PlayerId,
        difficulty: Difficulty,
    ) -> Result<(), RequestError> {
        let mut stream = self
            .client
            .start_game(StartGameRequest {
                machine_id: self.machine_id.into_inner(),
                difficulty: difficulty.into(),
                player_id: player_id.into_inner(),
            })
            .await?
            .into_inner();
        let resp = stream
            .next()
            .await
            .ok_or(RequestError::UnexpectedEndOfStream)??;
        let event = resp
            .event
            .ok_or(RequestError::missing_field("StartGameResponse", "event"))?;
        let started = event
            .event_data
            .as_ref()
            .ok_or(RequestError::missing_field("Event", "event_data"))?;
        let EventData::GameStarted(started) = started else {
            return Err(RequestError::UnexpectedEvent(event));
        };
        {
            let difficulty: i32 = difficulty.into();
            assert_eq!(
                started.difficulty, difficulty,
                "difficulty in request and in response should be same"
            );
        }

        let (rem_tx, rem_rx) = watch::channel(DisplayableRemainingTime::ZERO);
        let (score_tx, score_rx) = watch::channel(0);
        let (error_tx, error_rx) = watch::channel(None);
        {
            let mut guard = self.session.write();
            *guard = Some(Session {
                difficulty: difficulty,
                rem_rx,
                score_rx,
            });
        }

        tokio::spawn(handle_subsequent_events(stream, error_tx, rem_tx));

        Ok(())
    }
}

/// Handles events from game-master's gRPC server after receiving `GameStarted` event.
///
/// error_tx's newly-sent value is always `Some`.
async fn handle_subsequent_events<S>(
    mut stream: S,
    error_tx: watch::Sender<Option<RequestError>>,
    rem_tx: watch::Sender<DisplayableRemainingTime>,
) where
    S: Stream<Item = Result<StartGameResponse, Status>> + Unpin,
{
    loop {
        let resp = match stream.next().await {
            Some(Ok(v)) => v,
            Some(Err(e)) => {
                error_tx.send(Some(RequestError::Grpc(e))).unwrap();
                continue;
            }
            None => {
                error_tx
                    .send(Some(RequestError::UnexpectedEndOfStream))
                    .unwrap();
                break;
            }
        };
        let Some(ev) = resp.event else {
            error_tx
                .send(Some(RequestError::missing_field(
                    "StartGameResponse",
                    "event",
                )))
                .unwrap();
            continue;
        };
        let Some(ev_data) = ev.event_data else {
            error_tx
                .send(Some(RequestError::missing_field("EventData", "event_data")))
                .unwrap();
            continue;
        };
        match ev_data {
            EventData::GameStarted(_) => {
                error_tx
                    .send(Some(RequestError::UnexpectedEvent(ev)))
                    .unwrap();
                continue;
            }
            EventData::GameTimeLimitNotify(v) => {
                let Some(rem) = v.remaining else {
                    error_tx
                        .send(Some(RequestError::missing_field(
                            "GameTimeLimitNotify",
                            "remaining",
                        )))
                        .unwrap();
                    continue;
                };
                let Ok(rem) = rem.try_into() else {
                    error_tx
                        .send(Some(RequestError::invalid_packet(
                            "GameTimeLimitNotify",
                            "remaining",
                            "the value was negative",
                        )))
                        .unwrap();
                    continue;
                };
                rem_tx.send(rem).unwrap();
            }
            EventData::GameFinished(_) => todo!(),
        }
    }
}

/// States of a game session.
#[derive(Debug, Clone, Getters)]
pub struct Session {
    difficulty: struckout_proto::Difficulty,
    score_rx: watch::Receiver<u32>,
    rem_rx: watch::Receiver<DisplayableRemainingTime>,
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use async_stream::stream;
    use struckout_proto::event::{EventData, GameStarted, GameTimeLimitNotify};
    use tokio::time::timeout;

    use crate::data::GameId;

    use super::*;

    #[derive(Clone)]
    struct FakeGrpcClient {}

    impl GameMasterGrpcClient for FakeGrpcClient {
        async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
        where
            D: TryInto<tonic::transport::Endpoint>,
            D::Error: Into<Box<dyn std::error::Error + Send + Sync + 'static>>,
        {
            Ok(Self {})
        }

        async fn add_player(
            &mut self,
            request: impl tonic::IntoRequest<AddPlayerRequest>,
        ) -> Result<Response<AddPlayerResponse>, Status> {
            todo!()
        }

        async fn start_game(
            &mut self,
            request: impl tonic::IntoRequest<StartGameRequest>,
        ) -> Result<
            Response<
                impl Stream<Item = Result<StartGameResponse, Status>> + Unpin + Send + Sync + 'static,
            >,
            Status,
        > {
            let s = stream! {
                yield EventData::GameStarted(GameStarted {
                    difficulty: struckout_proto::Difficulty::Normal.into(),
                });
                tokio::time::sleep(Duration::from_secs(10)).await;
                yield EventData::GameTimeLimitNotify(GameTimeLimitNotify {
                    remaining: Some(prost_types::Duration {
                        seconds: 100,
                        nanos: 0,
                    }),
                });
            };
            let s = Box::pin(s);
            let s = s.map(|ev| {
                Ok(StartGameResponse {
                    event: Some(struckout_proto::Event {
                        machine_id: MachineId(1).into_inner(),
                        game_id: GameId(5).into_inner(),
                        event_data: Some(ev),
                    }),
                })
            });
            Ok(Response::new(s))
        }
    }

    #[tokio::test]
    async fn connect_returns_invalid_server_addr_when_addr_is_invalid() {
        let addr = "256.256.256.256";
        let err = GameMasterClient::<FakeGrpcClient>::connect(addr, MachineId(1))
            .await
            .expect_err("should return error");
        let ConnectError::InvalidServerAddr {
            server_addr: addr_got,
            source: _,
        } = err
        else {
            panic!("error kind didn't match");
        };
        assert_eq!(addr_got, addr);
    }

    #[tokio::test]
    async fn start_game_returns_immediately_after_first_reponse() {
        let player_id = PlayerId(12);
        let difficulty = struckout_proto::Difficulty::Normal;
        let mut gm = GameMasterClient::<FakeGrpcClient>::connect("127.0.0.1", MachineId(1))
            .await
            .unwrap();

        let res = timeout(Duration::from_secs(1), gm.start_game(player_id, difficulty)).await;
        let res = res.expect("should not timeout");
        assert!(res.is_ok());
    }
}
