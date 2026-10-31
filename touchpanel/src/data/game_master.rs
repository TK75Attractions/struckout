use std::sync::Arc;

use futures_util::{Stream, StreamExt};
use struckout_proto::{
    AddPlayerRequest, Difficulty, GameMode, GetPlayerRequest, GoHomeScreenRequest,
    ListenEventsRequest, ListenEventsResponse, SetGameModeRequest, SetSessionDifficultyRequest,
    SetSessionReadyRequest, StartSessionRequest, ValidatePlayerNameRequest,
    game_master_player_service_client::GameMasterPlayerServiceClient,
    game_master_session_service_client::GameMasterSessionServiceClient,
    types::{MachineId, PlayerId},
    validate_player_name_response::ValidatePlayerNameResp,
};
use thiserror::Error;
use tokio::sync::watch;
use tokio_util::sync::{CancellationToken, DropGuard};
use tonic::{Status, transport::Channel};
use tracing::{instrument, trace};

use super::{Session, SessionPhase};
use crate::GAME_MASTER_GRPC_PORT;

#[derive(Debug, Clone)]
pub struct GameMasterClient {
    machine_id: MachineId,
    player_client: GameMasterPlayerServiceClient<Channel>,
    session_client: GameMasterSessionServiceClient<Channel>,
    session: watch::Sender<Session>,
    // The subscription ends when the last client is dropped, including on failed initialization.
    _subscription: Arc<DropGuard>,
}

#[derive(Debug, Error)]
pub enum ConnectError {
    #[error(transparent)]
    Transport(#[from] tonic::transport::Error),
    #[error(transparent)]
    Subscribe(#[from] Status),
}

#[derive(Debug, Clone, Error)]
pub enum RequestError {
    #[error(transparent)]
    Grpc(#[from] Status),
    #[error("イベントの受信が終了しました")]
    UnexpectedEndOfStream,
    #[error("field {field_name} of message {message_name} was invalid: {detail}")]
    InvalidPacket {
        message_name: String,
        field_name: String,
        detail: String,
    },
}

impl RequestError {
    pub(super) fn missing_field(message_name: &str, field_name: &str) -> Self {
        Self::invalid_packet(message_name, field_name, "required field was missing")
    }

    pub(super) fn invalid_packet(message_name: &str, field_name: &str, detail: &str) -> Self {
        Self::InvalidPacket {
            message_name: message_name.into(),
            field_name: field_name.into(),
            detail: detail.into(),
        }
    }
}

impl GameMasterClient {
    /// Subscribes before returning so readiness cannot race event subscription.
    #[instrument]
    pub async fn connect(server_addr: &str, machine_id: MachineId) -> Result<Self, ConnectError> {
        let channel = tonic::transport::Endpoint::new(format!(
            "http://{server_addr}:{GAME_MASTER_GRPC_PORT}"
        ))?
        .connect()
        .await?;
        Self::from_channel(channel, machine_id)
            .await
            .map_err(ConnectError::from)
    }

    async fn from_channel(channel: Channel, machine_id: MachineId) -> Result<Self, Status> {
        let player_client = GameMasterPlayerServiceClient::new(channel.clone());
        let mut session_client = GameMasterSessionServiceClient::new(channel);
        let stream = session_client
            .listen_events(ListenEventsRequest {
                machine_id: machine_id.into_inner(),
            })
            .await?
            .into_inner();
        let (session, _) = watch::channel(Session::default());
        let cancel = CancellationToken::new();
        tokio::spawn(listen_events(
            stream,
            machine_id,
            session.clone(),
            cancel.clone(),
        ));
        Ok(Self {
            machine_id,
            player_client,
            session_client,
            session,
            _subscription: Arc::new(cancel.drop_guard()),
        })
    }

    pub fn session(&self) -> watch::Receiver<Session> {
        self.session.subscribe()
    }

    #[instrument(skip(self))]
    pub async fn start_session(&mut self, player_id: PlayerId) -> Result<(), Status> {
        // Mode selection may be retried after StartSession succeeded but SetGameMode failed.
        if self.session.borrow().phase == SessionPhase::ConfiguringMode
            && self.session.borrow().player_id == Some(player_id)
        {
            return Ok(());
        }
        self.session_client
            .start_session(StartSessionRequest {
                machine_id: self.machine_id.into_inner(),
                player_id: player_id.into_inner(),
            })
            .await?;
        self.session.send_replace(Session {
            phase: SessionPhase::ConfiguringMode,
            player_id: Some(player_id),
            ..Session::default()
        });
        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn set_game_mode(&mut self, mode: GameMode) -> Result<(), Status> {
        self.session_client
            .set_game_mode(SetGameModeRequest {
                machine_id: self.machine_id.into_inner(),
                mode: mode.into(),
            })
            .await?;
        self.session
            .send_modify(|session| session.phase = SessionPhase::ConfiguringDifficulty);
        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn set_session_difficulty(&mut self, difficulty: Difficulty) -> Result<(), Status> {
        self.session_client
            .set_session_difficulty(SetSessionDifficultyRequest {
                machine_id: self.machine_id.into_inner(),
                difficulty: difficulty.into(),
            })
            .await?;
        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn set_session_ready(&mut self) -> Result<(), Status> {
        self.session_client
            .set_session_ready(SetSessionReadyRequest {
                machine_id: self.machine_id.into_inner(),
            })
            .await?;
        self.session.send_modify(|session| {
            // GameStarted (or even GameFinished) may arrive before the unary response.
            if session.phase == SessionPhase::ConfiguringDifficulty {
                session.phase = SessionPhase::Ready;
            }
        });
        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn go_home_screen(&mut self) -> Result<(), Status> {
        self.session_client
            .go_home_screen(GoHomeScreenRequest {
                machine_id: self.machine_id.into_inner(),
            })
            .await?;
        self.session.send_replace(Session::default());
        Ok(())
    }

    pub async fn add_player(&mut self, name: impl Into<String>) -> Result<PlayerId, Status> {
        self.player_client
            .add_player(AddPlayerRequest { name: name.into() })
            .await
            .map(|res| res.into_inner().player_id.into())
    }

    pub async fn validate_player_name(
        &mut self,
        name: impl Into<String>,
    ) -> Result<ValidatePlayerNameResp, RequestError> {
        self.player_client
            .validate_player_name(ValidatePlayerNameRequest {
                player_name: name.into(),
            })
            .await?
            .into_inner()
            .validate_player_name_resp
            .ok_or_else(|| {
                RequestError::missing_field(
                    "ValidatePlayerNameResponse",
                    "validate_player_name_resp",
                )
            })
    }

    pub async fn get_player(
        &mut self,
        name: impl Into<String>,
    ) -> Result<Option<PlayerId>, RequestError> {
        match self
            .player_client
            .get_player(GetPlayerRequest { name: name.into() })
            .await
        {
            Ok(res) => Ok(Some(res.into_inner().player_id.into())),
            Err(error) if error.code() == tonic::Code::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    /// Reuses the named player across games, including when another client creates it concurrently.
    pub async fn get_or_add_player(&mut self, name: &str) -> Result<PlayerId, RequestError> {
        if let Some(player_id) = self.get_player(name).await? {
            return Ok(player_id);
        }
        match self.add_player(name).await {
            Ok(player_id) => Ok(player_id),
            Err(error) if error.code() == tonic::Code::AlreadyExists => self
                .get_player(name)
                .await?
                .ok_or_else(|| Status::not_found("created player was not found").into()),
            Err(error) => Err(error.into()),
        }
    }
}

async fn listen_events<S>(
    mut stream: S,
    machine_id: MachineId,
    session: watch::Sender<Session>,
    cancel: CancellationToken,
) where
    S: Stream<Item = Result<ListenEventsResponse, Status>> + Unpin,
{
    loop {
        let response = tokio::select! {
            _ = cancel.cancelled() => return,
            response = stream.next() => response,
        };
        trace!(?response, "received event");
        let result = match response {
            Some(Ok(response)) => {
                let result = response
                    .event
                    .ok_or_else(|| RequestError::missing_field("ListenEventsResponse", "event"));
                result.and_then(|event| {
                    let mut result = Ok(());
                    session.send_modify(|session| result = session.apply_event(machine_id, event));
                    result
                })
            }
            Some(Err(error)) => Err(error.into()),
            None => Err(RequestError::UnexpectedEndOfStream),
        };
        if let Err(error) = result {
            session.send_modify(|session| session.error = Some(error));
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        pin::Pin,
        sync::atomic::{AtomicBool, Ordering},
        time::Duration,
    };

    use struckout_proto::{
        self as proto,
        event::{EventData, GameFinished, GameStarted},
        game_master_player_service_server::{
            GameMasterPlayerService, GameMasterPlayerServiceServer,
        },
        game_master_session_service_server::{
            GameMasterSessionService, GameMasterSessionServiceServer,
        },
        player_info::{Multi, PlayerInfoData, PlayerItem, Single},
    };
    use tokio::sync::{Notify, broadcast, mpsc};
    use tonic::{Request, Response, transport::Server};

    use super::*;

    #[derive(Debug, PartialEq)]
    enum Command {
        AddPlayer(String),
        ValidateName(String),
        FindPlayer(String),
        Start(u32, u32),
        Mode(u32, i32),
        Difficulty(u32, i32),
        Ready(u32),
        Home(u32),
    }

    struct FakePlayerService {
        commands: mpsc::UnboundedSender<Command>,
    }

    #[tonic::async_trait]
    impl GameMasterPlayerService for FakePlayerService {
        async fn add_player(
            &self,
            request: Request<proto::AddPlayerRequest>,
        ) -> Result<Response<proto::AddPlayerResponse>, Status> {
            self.commands
                .send(Command::AddPlayer(request.into_inner().name))
                .expect("test receiver is alive");
            Ok(Response::new(proto::AddPlayerResponse { player_id: 12 }))
        }

        async fn validate_player_name(
            &self,
            request: Request<proto::ValidatePlayerNameRequest>,
        ) -> Result<Response<proto::ValidatePlayerNameResponse>, Status> {
            let name = request.into_inner().player_name;
            self.commands
                .send(Command::ValidateName(name.clone()))
                .expect("test receiver is alive");
            let validate_player_name_resp = if name == "missing-field" {
                None
            } else {
                Some(
                    proto::validate_player_name_response::ValidatePlayerNameResp::Ok(
                        proto::validate_player_name_response::Ok {},
                    ),
                )
            };
            Ok(Response::new(proto::ValidatePlayerNameResponse {
                validate_player_name_resp,
            }))
        }

        async fn get_player(
            &self,
            request: Request<proto::GetPlayerRequest>,
        ) -> Result<Response<proto::GetPlayerResponse>, Status> {
            let name = request.into_inner().name;
            self.commands
                .send(Command::FindPlayer(name.clone()))
                .expect("test receiver is alive");
            if name == "missing" || name == "NoName403" {
                return Err(Status::not_found("player does not exist"));
            }
            Ok(Response::new(proto::GetPlayerResponse {
                player_id: 12,
                player_name: name,
            }))
        }
    }

    struct FakeSessionService {
        commands: mpsc::UnboundedSender<Command>,
        events: broadcast::Sender<ListenEventsResponse>,
        ready_response: Arc<Notify>,
        fail_next_navigation: Arc<AtomicBool>,
    }

    #[tonic::async_trait]
    impl GameMasterSessionService for FakeSessionService {
        type ListenEventsStream =
            Pin<Box<dyn Stream<Item = Result<ListenEventsResponse, Status>> + Send>>;
        async fn listen_events(
            &self,
            request: Request<ListenEventsRequest>,
        ) -> Result<Response<Self::ListenEventsStream>, Status> {
            assert_eq!(request.into_inner().machine_id, 1);
            let mut events = self.events.subscribe();
            Ok(Response::new(Box::pin(async_stream::stream! {
                while let Ok(event) = events.recv().await { yield Ok(event); }
            })))
        }
        async fn start_session(
            &self,
            request: Request<StartSessionRequest>,
        ) -> Result<Response<proto::StartSessionResponse>, Status> {
            let request = request.into_inner();
            self.commands
                .send(Command::Start(request.machine_id, request.player_id))
                .expect("test receiver is alive");
            Ok(Response::new(proto::StartSessionResponse {}))
        }
        async fn set_game_mode(
            &self,
            request: Request<SetGameModeRequest>,
        ) -> Result<Response<proto::SetGameModeResponse>, Status> {
            let request = request.into_inner();
            self.commands
                .send(Command::Mode(request.machine_id, request.mode))
                .expect("test receiver is alive");
            Ok(Response::new(proto::SetGameModeResponse {}))
        }
        async fn set_session_difficulty(
            &self,
            request: Request<SetSessionDifficultyRequest>,
        ) -> Result<Response<proto::SetSessionDifficultyResponse>, Status> {
            let request = request.into_inner();
            self.commands
                .send(Command::Difficulty(request.machine_id, request.difficulty))
                .expect("test receiver is alive");
            self.events
                .send(response(EventData::SessionDifficultyChanged(
                    proto::event::SessionDifficultyChanged {
                        difficulty: request.difficulty,
                    },
                )))
                .expect("client subscribed before configuring");
            Ok(Response::new(proto::SetSessionDifficultyResponse {}))
        }
        async fn set_session_ready(
            &self,
            request: Request<SetSessionReadyRequest>,
        ) -> Result<Response<proto::SetSessionReadyResponse>, Status> {
            self.commands
                .send(Command::Ready(request.into_inner().machine_id))
                .expect("test receiver is alive");
            self.ready_response.notified().await;
            Ok(Response::new(proto::SetSessionReadyResponse {}))
        }
        async fn go_home_screen(
            &self,
            request: Request<GoHomeScreenRequest>,
        ) -> Result<Response<proto::GoHomeScreenResponse>, Status> {
            self.commands
                .send(Command::Home(request.into_inner().machine_id))
                .expect("test receiver is alive");
            if self.fail_next_navigation.swap(false, Ordering::SeqCst) {
                return Err(Status::unavailable("event subscribers disconnected"));
            }
            Ok(Response::new(proto::GoHomeScreenResponse {}))
        }
        async fn get_game_result(
            &self,
            _: Request<proto::GetGameResultRequest>,
        ) -> Result<Response<proto::GetGameResultResponse>, Status> {
            panic!("get_game_result must not be called in this test")
        }
        async fn get_ranking(
            &self,
            _: Request<proto::GetRankingRequest>,
        ) -> Result<Response<proto::GetRankingResponse>, Status> {
            panic!("get_ranking must not be called in this test")
        }
        async fn get_games_by_player(
            &self,
            _: Request<proto::GetGamesByPlayerRequest>,
        ) -> Result<Response<proto::GetGamesByPlayerResponse>, Status> {
            panic!("get_games_by_player must not be called in this test")
        }
        async fn start_tutorial(
            &self,
            _: Request<proto::StartTutorialRequest>,
        ) -> Result<Response<proto::StartTutorialResponse>, Status> {
            panic!("start_tutorial must not be called in this test")
        }
        async fn go_previous_tutorial(
            &self,
            _: Request<proto::GoPreviousTutorialRequest>,
        ) -> Result<Response<proto::GoPreviousTutorialResponse>, Status> {
            panic!("go_previous_tutorial must not be called in this test")
        }
        async fn go_next_tutorial(
            &self,
            _: Request<proto::GoNextTutorialRequest>,
        ) -> Result<Response<proto::GoNextTutorialResponse>, Status> {
            panic!("go_next_tutorial must not be called in this test")
        }
        async fn add_target(
            &self,
            _: Request<proto::AddTargetRequest>,
        ) -> Result<Response<proto::AddTargetResponse>, Status> {
            panic!("add_target must not be called in this test")
        }
        async fn remove_target(
            &self,
            _: Request<proto::RemoveTargetRequest>,
        ) -> Result<Response<proto::RemoveTargetResponse>, Status> {
            panic!("remove_target must not be called in this test")
        }
        async fn hit_target(
            &self,
            _: Request<proto::HitTargetRequest>,
        ) -> Result<Response<proto::HitTargetResponse>, Status> {
            panic!("hit_target must not be called in this test")
        }
        async fn reset_combo(
            &self,
            _: Request<proto::ResetComboRequest>,
        ) -> Result<Response<proto::ResetComboResponse>, Status> {
            panic!("reset_combo must not be called in this test")
        }
    }

    fn response(data: EventData) -> ListenEventsResponse {
        ListenEventsResponse {
            event: Some(proto::Event {
                event_data: Some(data),
            }),
        }
    }

    fn started(multi: bool) -> EventData {
        let player = PlayerItem {
            machine_id: 1,
            player_id: 12,
            player_name: "Alice".into(),
        };
        let players = if multi {
            PlayerInfoData::Multi(Multi {
                player_1: Some(PlayerItem {
                    machine_id: 2,
                    player_id: 34,
                    player_name: "Bob".into(),
                }),
                player_2: Some(player),
            })
        } else {
            PlayerInfoData::Single(Single {
                player: Some(player),
            })
        };
        EventData::GameStarted(GameStarted {
            difficulty: Difficulty::Hard.into(),
            game_dur_secs: 90,
            player_info: Some(proto::PlayerInfo {
                player_info_data: Some(players),
            }),
        })
    }

    async fn wait_for(session: &mut watch::Receiver<Session>, check: impl Fn(&Session) -> bool) {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if check(&session.borrow_and_update()) {
                    return;
                }
                session
                    .changed()
                    .await
                    .expect("subscription remains active");
            }
        })
        .await
        .expect("state should be updated");
    }

    struct Fixture {
        client: GameMasterClient,
        commands: mpsc::UnboundedReceiver<Command>,
        events: broadcast::Sender<ListenEventsResponse>,
        ready_response: Arc<Notify>,
        fail_next_navigation: Arc<AtomicBool>,
        shutdown: CancellationToken,
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            self.shutdown.cancel();
        }
    }
    impl Fixture {
        async fn new() -> Self {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("test port should be available");
            let address = listener.local_addr().expect("listener has an address");
            let (commands_tx, commands) = mpsc::unbounded_channel();
            let (events, _) = broadcast::channel(32);
            let ready_response = Arc::new(Notify::new());
            let fail_next_navigation = Arc::new(AtomicBool::new(false));
            let shutdown = CancellationToken::new();
            let service = FakeSessionService {
                commands: commands_tx.clone(),
                events: events.clone(),
                ready_response: ready_response.clone(),
                fail_next_navigation: fail_next_navigation.clone(),
            };
            let player_service = FakePlayerService {
                commands: commands_tx,
            };
            let incoming = async_stream::stream! {
                loop { yield Ok::<_, std::io::Error>(listener.accept().await.expect("test listener remains open").0); }
            };
            let cancel = shutdown.clone();
            tokio::spawn(async move {
                Server::builder()
                    .add_service(GameMasterSessionServiceServer::new(service))
                    .add_service(GameMasterPlayerServiceServer::new(player_service))
                    .serve_with_incoming_shutdown(incoming, cancel.cancelled_owned())
                    .await
                    .expect("test server should run");
            });
            let channel = tonic::transport::Endpoint::new(format!("http://{address}"))
                .expect("valid test endpoint")
                .connect()
                .await
                .expect("test server is listening");
            // No event has been emitted; connecting must not wait for GameStarted.
            let client = tokio::time::timeout(
                Duration::from_secs(3),
                GameMasterClient::from_channel(channel, MachineId::new(1)),
            )
            .await
            .expect("subscription should complete without an event")
            .expect("test subscription succeeds");
            Self {
                client,
                commands,
                events,
                ready_response,
                fail_next_navigation,
                shutdown,
            }
        }
        async fn configure(&mut self, mode: GameMode) {
            self.client
                .start_session(PlayerId::new(12))
                .await
                .expect("start session");
            self.client.set_game_mode(mode).await.expect("set mode");
            self.client
                .set_session_difficulty(Difficulty::Normal)
                .await
                .expect("set difficulty");
            assert_eq!(self.commands.recv().await, Some(Command::Start(1, 12)));
            assert_eq!(
                self.commands.recv().await,
                Some(Command::Mode(1, mode.into()))
            );
            assert_eq!(
                self.commands.recv().await,
                Some(Command::Difficulty(1, Difficulty::Normal.into()))
            );
        }
    }

    #[tokio::test]
    async fn player_requests_use_the_separate_player_service() {
        let mut fixture = Fixture::new().await;
        assert_eq!(
            fixture
                .client
                .add_player("new")
                .await
                .expect("player added"),
            PlayerId::new(12)
        );
        assert_eq!(
            fixture.commands.recv().await,
            Some(Command::AddPlayer("new".into()))
        );

        let validation = fixture
            .client
            .validate_player_name("new")
            .await
            .expect("valid name response");
        assert!(matches!(validation, ValidatePlayerNameResp::Ok(_)));
        assert_eq!(
            fixture.commands.recv().await,
            Some(Command::ValidateName("new".into()))
        );

        assert_eq!(
            fixture
                .client
                .get_player("existing")
                .await
                .expect("player found"),
            Some(PlayerId::new(12))
        );
        assert_eq!(
            fixture.commands.recv().await,
            Some(Command::FindPlayer("existing".into()))
        );
        assert_eq!(
            fixture
                .client
                .get_player("missing")
                .await
                .expect("not found is expected"),
            None
        );
        assert_eq!(
            fixture.commands.recv().await,
            Some(Command::FindPlayer("missing".into()))
        );

        assert!(matches!(
            fixture.client.validate_player_name("missing-field").await,
            Err(RequestError::InvalidPacket { .. })
        ));
        assert_eq!(
            fixture.commands.recv().await,
            Some(Command::ValidateName("missing-field".into()))
        );
    }

    #[tokio::test]
    async fn reuses_existing_player_and_adds_missing_player() {
        let mut fixture = Fixture::new().await;
        assert_eq!(
            fixture.client.get_or_add_player("existing").await.unwrap(),
            PlayerId::new(12)
        );
        assert_eq!(
            fixture.commands.recv().await,
            Some(Command::FindPlayer("existing".into()))
        );

        assert_eq!(
            fixture.client.get_or_add_player("NoName403").await.unwrap(),
            PlayerId::new(12)
        );
        assert_eq!(
            fixture.commands.recv().await,
            Some(Command::FindPlayer("NoName403".into()))
        );
        assert_eq!(
            fixture.commands.recv().await,
            Some(Command::AddPlayer("NoName403".into()))
        );
    }

    #[rstest::rstest]
    #[case::single(false, GameMode::Single)]
    #[case::multi(true, GameMode::Multi)]
    #[tokio::test]
    async fn events_before_ready_response_are_retained(
        #[case] multi: bool,
        #[case] mode: GameMode,
    ) {
        let mut fixture = Fixture::new().await;
        fixture.configure(mode).await;
        let mut client = fixture.client.clone();
        let ready = tokio::spawn(async move { client.set_session_ready().await });
        assert_eq!(fixture.commands.recv().await, Some(Command::Ready(1)));
        fixture
            .events
            .send(response(started(multi)))
            .expect("subscribed");
        let mut session = fixture.client.session();
        wait_for(&mut session, |s| s.phase == SessionPhase::Running).await;
        assert_eq!(session.borrow().difficulty, Some(Difficulty::Hard));
        assert_eq!(session.borrow().remaining_time.to_string(), "01:30");
        fixture
            .events
            .send(response(EventData::GameFinished(GameFinished {
                game_id: 42,
            })))
            .expect("subscribed");
        wait_for(&mut session, |s| {
            s.phase == SessionPhase::Finished(struckout_proto::types::GameId::new(42))
        })
        .await;
        fixture.ready_response.notify_one();
        ready
            .await
            .expect("readiness task completes")
            .expect("readiness succeeds");
        // Both the response and a late UI subscription must preserve completion.
        assert_eq!(
            fixture.client.session().borrow().phase,
            SessionPhase::Finished(struckout_proto::types::GameId::new(42))
        );
    }

    #[tokio::test]
    async fn readiness_waits_for_an_event_and_subscription_survives_game_completion() {
        let mut fixture = Fixture::new().await;
        fixture.configure(GameMode::Multi).await;
        fixture.ready_response.notify_one();
        fixture
            .client
            .set_session_ready()
            .await
            .expect("readiness succeeds");
        assert_eq!(fixture.commands.recv().await, Some(Command::Ready(1)));
        assert_eq!(fixture.client.session().borrow().phase, SessionPhase::Ready);
        fixture
            .events
            .send(response(started(true)))
            .expect("subscribed");
        let mut session = fixture.client.session();
        wait_for(&mut session, |s| s.phase == SessionPhase::Running).await;
        fixture
            .events
            .send(response(EventData::GameFinished(GameFinished {
                game_id: 42,
            })))
            .expect("subscribed");
        wait_for(&mut session, |s| {
            s.phase == SessionPhase::Finished(struckout_proto::types::GameId::new(42))
        })
        .await;
        fixture.client.go_home_screen().await.expect("go home");
        assert_eq!(fixture.commands.recv().await, Some(Command::Home(1)));
        fixture.configure(GameMode::Single).await;
        fixture
            .events
            .send(response(started(false)))
            .expect("still subscribed");
        wait_for(&mut session, |s| s.phase == SessionPhase::Running).await;
    }

    #[tokio::test]
    async fn failed_screen_navigation_preserves_finished_state_for_retry() {
        let mut fixture = Fixture::new().await;
        fixture.configure(GameMode::Single).await;
        fixture.ready_response.notify_one();
        fixture.client.set_session_ready().await.expect("ready");
        assert_eq!(fixture.commands.recv().await, Some(Command::Ready(1)));
        fixture
            .events
            .send(response(started(false)))
            .expect("subscribed");
        let mut session = fixture.client.session();
        wait_for(&mut session, |s| s.phase == SessionPhase::Running).await;
        fixture
            .events
            .send(response(EventData::GameFinished(GameFinished {
                game_id: 42,
            })))
            .expect("subscribed");
        wait_for(&mut session, |s| {
            s.phase == SessionPhase::Finished(struckout_proto::types::GameId::new(42))
        })
        .await;

        fixture.fail_next_navigation.store(true, Ordering::SeqCst);
        let error = fixture.client.go_home_screen().await.expect_err("fails");
        assert_eq!(error.code(), tonic::Code::Unavailable);
        assert_eq!(fixture.commands.recv().await, Some(Command::Home(1)));
        assert_eq!(
            session.borrow().phase,
            SessionPhase::Finished(struckout_proto::types::GameId::new(42))
        );
        fixture.client.go_home_screen().await.expect("retry home");
        assert_eq!(fixture.commands.recv().await, Some(Command::Home(1)));
        assert_eq!(session.borrow().phase, SessionPhase::Idle);
    }

    #[tokio::test]
    async fn stream_errors_and_missing_fields_are_reported() {
        for responses in [
            vec![],
            vec![Err(Status::internal("save failed"))],
            vec![Ok(ListenEventsResponse { event: None })],
        ] {
            let (state, _) = watch::channel(Session::default());
            let stream = futures_util::stream::iter(responses);
            listen_events(
                stream,
                MachineId::new(1),
                state.clone(),
                CancellationToken::new(),
            )
            .await;
            assert!(state.borrow().error.is_some());
        }
    }
}
