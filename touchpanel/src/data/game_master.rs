use std::{pin::Pin, sync::Arc, time::Duration};

use futures_util::{Stream, StreamExt};
use parking_lot::{RwLock, RwLockReadGuard};
use struckout_proto::{
    AddPlayerRequest, AddPlayerResponse, Difficulty, GameMode, GetGameResultRequest,
    GetGameResultResponse, GetPlayerRequest, GetPlayerResponse, ListenEventsRequest,
    ListenEventsResponse, SetGameModeRequest, SetGameModeResponse, SetSessionDifficultyRequest,
    SetSessionDifficultyResponse, SetSessionReadyRequest, SetSessionReadyResponse,
    StartSessionRequest, StartSessionResponse, ValidatePlayerNameRequest,
    ValidatePlayerNameResponse, event::EventData,
    game_master_player_service_client::GameMasterPlayerServiceClient,
    game_master_session_service_client::GameMasterSessionServiceClient, types::GameId,
    validate_player_name_response::ValidatePlayerNameResp,
};
use thiserror::Error;
use tokio::sync::{broadcast, mpsc, watch};
use tokio_util::sync::CancellationToken;
use tonic::{Response, Status};
use tracing::{instrument, trace};

use crate::{GAME_MASTER_GRPC_PORT, data::remaining_time::DisplayableRemainingTime};

use struckout_proto::types::{MachineId, PlayerId};

mod private {
    pub trait Sealed {}
}

type EventStream = Pin<Box<dyn Stream<Item = Result<ListenEventsResponse, Status>> + Send>>;

/// A narrow adapter for the two game-master services so the data flow can be driven by a fake.
pub trait InternalGrpcClient: Sized + Sync + Send + Clone + private::Sealed {
    fn connect<D>(dst: D) -> impl Future<Output = Result<Self, tonic::transport::Error>>
    where
        D: TryInto<tonic::transport::Endpoint>,
        D::Error: Into<Box<dyn std::error::Error + Send + Sync + 'static>>;

    fn add_player(
        &mut self,
        request: impl tonic::IntoRequest<AddPlayerRequest>,
    ) -> impl Future<Output = Result<Response<AddPlayerResponse>, Status>>;

    fn validate_player_name(
        &mut self,
        request: impl tonic::IntoRequest<ValidatePlayerNameRequest>,
    ) -> impl Future<Output = Result<Response<ValidatePlayerNameResponse>, Status>>;

    fn get_player(
        &mut self,
        request: impl tonic::IntoRequest<GetPlayerRequest>,
    ) -> impl Future<Output = Result<Response<GetPlayerResponse>, Status>>;

    fn start_session(
        &mut self,
        request: impl tonic::IntoRequest<StartSessionRequest>,
    ) -> impl Future<Output = Result<Response<StartSessionResponse>, Status>>;

    fn set_game_mode(
        &mut self,
        request: impl tonic::IntoRequest<SetGameModeRequest>,
    ) -> impl Future<Output = Result<Response<SetGameModeResponse>, Status>>;

    fn set_session_difficulty(
        &mut self,
        request: impl tonic::IntoRequest<SetSessionDifficultyRequest>,
    ) -> impl Future<Output = Result<Response<SetSessionDifficultyResponse>, Status>>;

    fn set_session_ready(
        &mut self,
        request: impl tonic::IntoRequest<SetSessionReadyRequest>,
    ) -> impl Future<Output = Result<Response<SetSessionReadyResponse>, Status>>;

    fn listen_events(
        &mut self,
        request: impl tonic::IntoRequest<ListenEventsRequest>,
    ) -> impl Future<Output = Result<EventStream, Status>>;

    fn get_game_result(
        &mut self,
        request: impl tonic::IntoRequest<GetGameResultRequest>,
    ) -> impl Future<Output = Result<Response<GetGameResultResponse>, Status>>;
}

#[derive(Clone)]
pub struct GameMasterGrpcClient {
    player: GameMasterPlayerServiceClient<tonic::transport::Channel>,
    session: GameMasterSessionServiceClient<tonic::transport::Channel>,
}

impl std::fmt::Debug for GameMasterGrpcClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("GameMasterGrpcClient").finish()
    }
}

impl private::Sealed for GameMasterGrpcClient {}

impl InternalGrpcClient for GameMasterGrpcClient {
    async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
    where
        D: TryInto<tonic::transport::Endpoint>,
        D::Error: Into<Box<dyn std::error::Error + Send + Sync + 'static>>,
    {
        let channel = tonic::transport::Endpoint::new(dst)?.connect().await?;
        Ok(Self {
            player: GameMasterPlayerServiceClient::new(channel.clone()),
            session: GameMasterSessionServiceClient::new(channel),
        })
    }

    async fn add_player(
        &mut self,
        request: impl tonic::IntoRequest<AddPlayerRequest>,
    ) -> Result<Response<AddPlayerResponse>, Status> {
        self.player.add_player(request).await
    }

    async fn validate_player_name(
        &mut self,
        request: impl tonic::IntoRequest<ValidatePlayerNameRequest>,
    ) -> Result<Response<ValidatePlayerNameResponse>, Status> {
        self.player.validate_player_name(request).await
    }

    async fn get_player(
        &mut self,
        request: impl tonic::IntoRequest<GetPlayerRequest>,
    ) -> Result<Response<GetPlayerResponse>, Status> {
        self.player.get_player(request).await
    }

    async fn start_session(
        &mut self,
        request: impl tonic::IntoRequest<StartSessionRequest>,
    ) -> Result<Response<StartSessionResponse>, Status> {
        self.session.start_session(request).await
    }

    async fn set_game_mode(
        &mut self,
        request: impl tonic::IntoRequest<SetGameModeRequest>,
    ) -> Result<Response<SetGameModeResponse>, Status> {
        self.session.set_game_mode(request).await
    }

    async fn set_session_difficulty(
        &mut self,
        request: impl tonic::IntoRequest<SetSessionDifficultyRequest>,
    ) -> Result<Response<SetSessionDifficultyResponse>, Status> {
        self.session.set_session_difficulty(request).await
    }

    async fn set_session_ready(
        &mut self,
        request: impl tonic::IntoRequest<SetSessionReadyRequest>,
    ) -> Result<Response<SetSessionReadyResponse>, Status> {
        self.session.set_session_ready(request).await
    }

    async fn listen_events(
        &mut self,
        request: impl tonic::IntoRequest<ListenEventsRequest>,
    ) -> Result<EventStream, Status> {
        let stream = self.session.listen_events(request).await?.into_inner();
        Ok(Box::pin(stream))
    }

    async fn get_game_result(
        &mut self,
        request: impl tonic::IntoRequest<GetGameResultRequest>,
    ) -> Result<Response<GetGameResultResponse>, Status> {
        self.session.get_game_result(request).await
    }
}

#[derive(derive_more::Debug, Clone)]
pub struct GameMasterClient<T: InternalGrpcClient> {
    machine_id: MachineId,
    client: T,
    session: Arc<RwLock<Option<Session>>>,
}

/// Error returned from [`GameMasterClient::connect()`].
#[derive(Debug, Error)]
pub enum ConnectError {
    #[error(transparent)]
    Other(#[from] tonic::transport::Error),
}

/// Returned from [`GameMasterClient`]'s method which sends request to server.
#[derive(Debug, Error)]
pub enum RequestError {
    #[error(transparent)]
    Grpc(#[from] Status),
    #[error("event stream unexpectedly ended")]
    UnexpectedEndOfStream,
    #[error("game did not start within the allowed time")]
    StartTimedOut,
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
    fn missing_field(message_name: &str, field_name: &str) -> Self {
        Self::InvalidPacket {
            message_name: message_name.to_string(),
            field_name: field_name.to_string(),
            detail: "field was missing".to_string(),
        }
    }

    fn invalid_packet(message_name: &str, field_name: &str, detail: &str) -> Self {
        Self::InvalidPacket {
            message_name: message_name.to_string(),
            field_name: field_name.to_string(),
            detail: detail.to_string(),
        }
    }
}

impl<T: InternalGrpcClient> GameMasterClient<T> {
    pub async fn connect(server_addr: &str, machine_id: MachineId) -> Result<Self, ConnectError> {
        let endpoint = format!("http://{}:{}", server_addr, GAME_MASTER_GRPC_PORT);
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
            .map(|response| response.into_inner().player_id.into())
    }

    /// Runs the single-game session protocol and waits for this Ready response's game ID.
    #[instrument(skip(self))]
    pub async fn start_game(
        &mut self,
        player_id: PlayerId,
        difficulty: Difficulty,
    ) -> Result<GameId, RequestError> {
        let stream = self
            .client
            .listen_events(ListenEventsRequest {
                machine_id: self.machine_id.into_inner(),
            })
            .await?;
        let cancellation = CancellationToken::new();
        let (event_tx, mut event_rx) = mpsc::channel(32);
        tokio::spawn(forward_events(stream, event_tx, cancellation.child_token()));

        let machine_id = self.machine_id.into_inner();
        let setup = async {
            self.client
                .start_session(StartSessionRequest {
                    machine_id,
                    player_id: player_id.into_inner(),
                })
                .await?;
            self.client
                .set_game_mode(SetGameModeRequest {
                    mode: GameMode::Single.into(),
                    machine_id,
                })
                .await?;
            self.client
                .set_session_difficulty(SetSessionDifficultyRequest {
                    difficulty: difficulty.into(),
                    machine_id,
                })
                .await?;
            self.client
                .set_session_ready(SetSessionReadyRequest { machine_id })
                .await
                .map(|response| response.into_inner().game_id.into())
                .map_err(RequestError::from)
        }
        .await;

        let game_id: GameId = match setup {
            Ok(game_id) => game_id,
            Err(error) => {
                cancellation.cancel();
                return Err(error);
            }
        };

        let started = tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let response = event_rx
                    .recv()
                    .await
                    .ok_or(RequestError::UnexpectedEndOfStream)??;
                let event = response
                    .event
                    .ok_or(RequestError::missing_field("ListenEventsResponse", "event"))?;
                let event_data = event
                    .event_data
                    .ok_or_else(|| RequestError::missing_field("Event", "event_data"))?;
                if let EventData::GameStarted(started) = event_data {
                    if started.game_id == game_id.into_inner() {
                        let actual_difficulty =
                            Difficulty::try_from(started.difficulty).map_err(|_| {
                                RequestError::invalid_packet(
                                    "GameStarted",
                                    "difficulty",
                                    "unknown enum value",
                                )
                            })?;
                        if actual_difficulty != difficulty {
                            return Err(RequestError::invalid_packet(
                                "GameStarted",
                                "difficulty",
                                "did not match the request",
                            ));
                        }
                        break Ok(());
                    }
                }
            }
        })
        .await;

        let started = match started {
            Ok(started) => started,
            Err(_) => {
                cancellation.cancel();
                return Err(RequestError::StartTimedOut);
            }
        };

        if let Err(error) = started {
            cancellation.cancel();
            return Err(error);
        }

        let (rem_tx, rem_rx) = watch::channel(DisplayableRemainingTime::ZERO);
        let (score_tx, score_rx) = watch::channel(0);
        let (error_tx, error_rx) = watch::channel(None);
        let (complete_tx, _) = broadcast::channel(1);
        let session = Session {
            game_id,
            difficulty,
            rem_tx,
            score_tx,
            error_tx,
            complete_tx,
            cancellation: cancellation.clone(),
        };
        *self.session.write() = Some(session.clone());

        tokio::spawn(handle_subsequent_events(
            event_rx,
            game_id,
            session,
            Arc::clone(&self.session),
        ));
        // Hold these receivers until the stream ends so the initial values are always retained.
        tokio::spawn(keep_receivers_alive(
            rem_rx,
            score_rx,
            error_rx,
            cancellation,
        ));

        Ok(game_id)
    }

    #[instrument(skip(self))]
    pub async fn get_game_result(&mut self, game_id: GameId) -> Result<u32, Status> {
        self.client
            .get_game_result(GetGameResultRequest {
                game_id: game_id.into_inner(),
            })
            .await
            .map(|response| response.into_inner().total_score)
    }

    pub async fn validate_player_name(
        &mut self,
        name: impl Into<String>,
    ) -> Result<ValidatePlayerNameResp, RequestError> {
        let response = self
            .client
            .validate_player_name(ValidatePlayerNameRequest {
                player_name: name.into(),
            })
            .await?
            .into_inner();
        response.validate_player_name_resp.ok_or_else(|| {
            RequestError::missing_field("ValidatePlayerNameResponse", "validate_player_name_resp")
        })
    }

    /// Returns an existing player ID, `None` for an unknown name, and an error for other failures.
    pub async fn get_player(
        &mut self,
        name: impl Into<String>,
    ) -> Result<Option<PlayerId>, RequestError> {
        match self
            .client
            .get_player(GetPlayerRequest { name: name.into() })
            .await
        {
            Ok(response) => Ok(Some(response.into_inner().player_id.into())),
            Err(error) if error.code() == tonic::Code::NotFound => Ok(None),
            Err(error) => Err(RequestError::Grpc(error)),
        }
    }

    pub fn session(&self) -> RwLockReadGuard<'_, Option<Session>> {
        self.session.read()
    }
}

async fn forward_events<S>(
    mut stream: S,
    tx: mpsc::Sender<Result<ListenEventsResponse, RequestError>>,
    cancellation: CancellationToken,
) where
    S: Stream<Item = Result<ListenEventsResponse, Status>> + Send + Unpin,
{
    loop {
        let received = tokio::select! {
            _ = cancellation.cancelled() => return,
            received = stream.next() => received,
        };
        let message = match received {
            Some(Ok(response)) => Ok(response),
            Some(Err(error)) => Err(RequestError::Grpc(error)),
            None => Err(RequestError::UnexpectedEndOfStream),
        };
        let finished = message.is_err();
        if tx.send(message).await.is_err() || finished {
            return;
        }
    }
}

async fn handle_subsequent_events(
    mut events: mpsc::Receiver<Result<ListenEventsResponse, RequestError>>,
    game_id: GameId,
    session: Session,
    current_session: Arc<RwLock<Option<Session>>>,
) {
    loop {
        let response = match events.recv().await {
            Some(Ok(response)) => response,
            Some(Err(error)) => {
                session.error_tx.send_replace(Some(error));
                break;
            }
            None => {
                session
                    .error_tx
                    .send_replace(Some(RequestError::UnexpectedEndOfStream));
                break;
            }
        };
        trace!(?response, "received event");
        let Some(event) = response.event else {
            session
                .error_tx
                .send_replace(Some(RequestError::missing_field(
                    "ListenEventsResponse",
                    "event",
                )));
            break;
        };
        let Some(event_data) = event.event_data else {
            session
                .error_tx
                .send_replace(Some(RequestError::missing_field("Event", "event_data")));
            break;
        };
        let event_game_id = match &event_data {
            EventData::GameStarted(value) => value.game_id,
            EventData::GameTimeLimitNotify(value) => value.game_id,
            EventData::GameScoreChanged(value) => value.game_id,
            EventData::GameFinished(value) => value.game_id,
        };
        if event_game_id != game_id.into_inner() {
            continue;
        }

        match event_data {
            EventData::GameStarted(_) => (),
            EventData::GameTimeLimitNotify(value) => {
                let Some(remaining) = value.remaining else {
                    session
                        .error_tx
                        .send_replace(Some(RequestError::missing_field(
                            "GameTimeLimitNotify",
                            "remaining",
                        )));
                    break;
                };
                match DisplayableRemainingTime::try_from(remaining) {
                    Ok(remaining) => {
                        session.rem_tx.send_replace(remaining);
                    }
                    Err(error) => {
                        session
                            .error_tx
                            .send_replace(Some(RequestError::invalid_packet(
                                "GameTimeLimitNotify",
                                "remaining",
                                &error.to_string(),
                            )));
                        break;
                    }
                }
            }
            EventData::GameScoreChanged(value) => {
                session.score_tx.send_replace(value.total_score);
            }
            EventData::GameFinished(_) => {
                let _ = session.complete_tx.send(());
                session.cancellation.cancel();
                clear_session(&current_session, game_id);
                return;
            }
        }
    }
    session.cancellation.cancel();
    clear_session(&current_session, game_id);
}

fn clear_session(current_session: &RwLock<Option<Session>>, game_id: GameId) {
    let mut current = current_session.write();
    if current
        .as_ref()
        .is_some_and(|session| session.game_id == game_id)
    {
        *current = None;
        trace!(%game_id, "removed current session");
    }
}

async fn keep_receivers_alive(
    mut rem_rx: watch::Receiver<DisplayableRemainingTime>,
    mut score_rx: watch::Receiver<u32>,
    mut error_rx: watch::Receiver<Option<RequestError>>,
    cancellation: CancellationToken,
) {
    loop {
        tokio::select! {
            _ = cancellation.cancelled() => return,
            changed = rem_rx.changed() => if changed.is_err() { return; },
            changed = score_rx.changed() => if changed.is_err() { return; },
            changed = error_rx.changed() => if changed.is_err() { return; },
        }
    }
}

/// State and watch channels for one game. The game ID prevents old completion tasks clearing a later game.
#[derive(Debug, Clone)]
pub struct Session {
    game_id: GameId,
    difficulty: Difficulty,
    score_tx: watch::Sender<u32>,
    rem_tx: watch::Sender<DisplayableRemainingTime>,
    error_tx: watch::Sender<Option<RequestError>>,
    complete_tx: broadcast::Sender<()>,
    cancellation: CancellationToken,
}

impl Session {
    /// Constructs a session state for presentation tests that exercise watch-channel behavior.
    pub fn new(
        difficulty: Difficulty,
        score_tx: watch::Sender<u32>,
        rem_tx: watch::Sender<DisplayableRemainingTime>,
        error_tx: watch::Sender<Option<RequestError>>,
        complete_tx: broadcast::Sender<()>,
    ) -> Self {
        Self {
            game_id: GameId::new(1),
            difficulty,
            score_tx,
            rem_tx,
            error_tx,
            complete_tx,
            cancellation: CancellationToken::new(),
        }
    }

    pub fn game_id(&self) -> GameId {
        self.game_id
    }

    pub fn difficulty(&self) -> Difficulty {
        self.difficulty
    }

    pub fn score(&self) -> watch::Receiver<u32> {
        self.score_tx.subscribe()
    }

    pub fn remaining_time(&self) -> watch::Receiver<DisplayableRemainingTime> {
        self.rem_tx.subscribe()
    }

    pub fn error(&self) -> watch::Receiver<Option<RequestError>> {
        self.error_tx.subscribe()
    }

    pub fn complete(&self) -> broadcast::Receiver<()> {
        self.complete_tx.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::stream;
    use struckout_proto::{Event, event::GameStarted, types::MachineId};
    use tokio::time::timeout;

    #[derive(Debug, Clone)]
    struct FakeGrpcClient;

    impl private::Sealed for FakeGrpcClient {}

    impl InternalGrpcClient for FakeGrpcClient {
        async fn connect<D>(_dst: D) -> Result<Self, tonic::transport::Error>
        where
            D: TryInto<tonic::transport::Endpoint>,
            D::Error: Into<Box<dyn std::error::Error + Send + Sync + 'static>>,
        {
            Ok(Self)
        }

        async fn add_player(
            &mut self,
            _request: impl tonic::IntoRequest<AddPlayerRequest>,
        ) -> Result<Response<AddPlayerResponse>, Status> {
            Ok(Response::new(AddPlayerResponse { player_id: 12 }))
        }

        async fn validate_player_name(
            &mut self,
            _request: impl tonic::IntoRequest<ValidatePlayerNameRequest>,
        ) -> Result<Response<ValidatePlayerNameResponse>, Status> {
            Ok(Response::new(ValidatePlayerNameResponse {
                validate_player_name_resp: Some(ValidatePlayerNameResp::Ok(
                    struckout_proto::validate_player_name_response::Ok {},
                )),
            }))
        }

        async fn get_player(
            &mut self,
            _request: impl tonic::IntoRequest<GetPlayerRequest>,
        ) -> Result<Response<GetPlayerResponse>, Status> {
            Ok(Response::new(GetPlayerResponse {
                player_id: 12,
                player_name: "test".into(),
            }))
        }

        async fn start_session(
            &mut self,
            _request: impl tonic::IntoRequest<StartSessionRequest>,
        ) -> Result<Response<StartSessionResponse>, Status> {
            Ok(Response::new(StartSessionResponse {}))
        }

        async fn set_game_mode(
            &mut self,
            _request: impl tonic::IntoRequest<SetGameModeRequest>,
        ) -> Result<Response<SetGameModeResponse>, Status> {
            Ok(Response::new(SetGameModeResponse {}))
        }

        async fn set_session_difficulty(
            &mut self,
            _request: impl tonic::IntoRequest<SetSessionDifficultyRequest>,
        ) -> Result<Response<SetSessionDifficultyResponse>, Status> {
            Ok(Response::new(SetSessionDifficultyResponse {}))
        }

        async fn set_session_ready(
            &mut self,
            _request: impl tonic::IntoRequest<SetSessionReadyRequest>,
        ) -> Result<Response<SetSessionReadyResponse>, Status> {
            Ok(Response::new(SetSessionReadyResponse { game_id: 5 }))
        }

        async fn listen_events(
            &mut self,
            _request: impl tonic::IntoRequest<ListenEventsRequest>,
        ) -> Result<EventStream, Status> {
            let events = [4, 5].map(|game_id| {
                Ok(ListenEventsResponse {
                    event: Some(Event {
                        event_data: Some(EventData::GameStarted(GameStarted {
                            difficulty: Difficulty::Normal.into(),
                            game_id,
                        })),
                    }),
                })
            });
            Ok(Box::pin(stream::iter(events)))
        }

        async fn get_game_result(
            &mut self,
            _request: impl tonic::IntoRequest<GetGameResultRequest>,
        ) -> Result<Response<GetGameResultResponse>, Status> {
            Ok(Response::new(GetGameResultResponse { total_score: 0 }))
        }
    }

    #[tokio::test]
    async fn start_game_waits_for_the_ready_game_id_not_an_old_snapshot() {
        let mut client =
            GameMasterClient::<FakeGrpcClient>::connect("127.0.0.1", MachineId::new(1))
                .await
                .unwrap();
        let game_id = timeout(
            Duration::from_secs(1),
            client.start_game(PlayerId::new(12), Difficulty::Normal),
        )
        .await
        .expect("the matching start event should be available")
        .unwrap();
        assert_eq!(game_id, GameId::new(5));
        assert_eq!(client.session().as_ref().unwrap().game_id(), game_id);
    }
}
