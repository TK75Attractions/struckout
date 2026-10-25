mod player;
pub use player::GameMasterPlayerServiceImpl;

#[cfg(test)]
mod tests;

use std::{pin::Pin, sync::Arc, time::Duration};

use parking_lot::RwLock;
use struckout_proto::{
    self, AddTargetRequest, AddTargetResponse, CompleteSessionRequest, CompleteSessionResponse,
    Difficulty, GameMode, GetGameResultRequest, GetGameResultResponse, GoNextTutorialRequest,
    GoNextTutorialResponse, GoPreviousTutorialRequest, GoPreviousTutorialResponse,
    HitTargetRequest, HitTargetResponse, ListenEventsRequest, ListenEventsResponse,
    RemoveTargetRequest, RemoveTargetResponse, SetGameModeRequest, SetGameModeResponse,
    SetSessionDifficultyRequest, SetSessionDifficultyResponse, SetSessionReadyRequest,
    SetSessionReadyResponse, StartSessionRequest, StartSessionResponse, StartTutorialRequest,
    StartTutorialResponse,
    game_master_session_service_server::GameMasterSessionService,
    target::Owner,
    types::{MachineId, PlayerId},
};
use time::{SignedDuration, ext::NumericalDuration};
use tokio::sync::{broadcast, mpsc};
use tokio_stream::{
    Stream, StreamExt,
    wrappers::{BroadcastStream, ReceiverStream},
};
use tonic::{Request, Response, Status};
use tracing::{instrument, trace, warn};
use uuid::Uuid;

use crate::{
    DataSource, DataSourceImpl,
    session::{HitTargetError, SessionError, Sessions, SetReadyAction},
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
        ev_target: EventTarget,
        mut complete_rx: mpsc::Receiver<()>,
    ) {
        complete_rx.recv().await.expect("complete channel closed");
        {
            let mut sessions = self.sessions.write();
            let machine_id = match ev_target {
                EventTarget::Single(machine_id) => machine_id,
                EventTarget::All => sessions.machine_ids().expect("multi-play has two sessions")[0],
            };
            sessions
                .finish_game(machine_id)
                .expect("only running games have a completion timer");
        }
        if self
            .event_tx
            .send(Event {
                target: ev_target,
                data: Ok(SuccessfulEvent::GameFinished),
            })
            .is_err()
        {
            warn!("all event receivers have been dropped");
        }
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

        let ev_target = {
            let mut sessions = self.sessions.write();
            sessions
                .update_difficulty(machine_id, difficulty)
                .map_err(|e| Status::failed_precondition(e.to_string()))?
        };

        if self
            .event_tx
            .send(Event {
                target: ev_target,
                data: Ok(SuccessfulEvent::SessionDifficultyChanged(difficulty)),
            })
            .is_err()
        {
            warn!("all event receiver has been dropped");
        }
        Ok(Response::new(SetSessionDifficultyResponse {}))
    }

    /// Selects a session's game mode before difficulty configuration can begin.
    ///
    /// Returns `InvalidArgument` for unspecified or unknown modes and
    /// `FailedPrecondition` for a missing session or an invalid session state.
    /// Errors leave session state unchanged. This operation emits no events.
    #[instrument]
    async fn set_game_mode(
        &self,
        req: Request<SetGameModeRequest>,
    ) -> Result<Response<SetGameModeResponse>, Status> {
        let req = req.into_inner();
        let SetGameModeRequest { mode, machine_id } = req;
        let mode = GameMode::try_from(mode)
            .map_err(|e| Status::invalid_argument(format!("GameMode is out of range: {e}")))?;
        let machine_id: MachineId = machine_id.into();

        self.sessions
            .write()
            .set_game_mode(machine_id, mode)
            .map_err(|e| match e {
                SessionError::UnspecifiedGameMode => Status::invalid_argument(e.to_string()),
                _ => Status::failed_precondition(e.to_string()),
            })?;

        Ok(Response::new(SetGameModeResponse {}))
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

    /// Adds target to the current session.
    ///
    /// Owner of a target created by this RPC (i.e. by `projector`) is always [`Owner::Single`].
    /// [`Owner::All`] can be used by special targets created by `game-master` internally.
    #[instrument]
    async fn add_target(
        &self,
        req: Request<AddTargetRequest>,
    ) -> Result<Response<AddTargetResponse>, Status> {
        let AddTargetRequest {
            machine_id,
            target_data,
        } = req.into_inner();
        let target_data =
            target_data.ok_or_else(|| Status::invalid_argument("target_data is required"))?;
        let target_data = match target_data {
            struckout_proto::add_target_request::TargetData::Compass(compass) => {
                struckout_proto::target::TargetData::Compass(compass)
            }
        };
        let target_id = self
            .sessions
            .write()
            .add_target(
                target_data,
                Owner::Single(struckout_proto::target::OwnerSingle { machine_id }),
            )
            .map_err(|e| Status::failed_precondition(e.to_string()))?;
        Ok(Response::new(AddTargetResponse {
            target_id: target_id.to_string(),
        }))
    }

    #[instrument]
    async fn remove_target(
        &self,
        req: Request<RemoveTargetRequest>,
    ) -> Result<Response<RemoveTargetResponse>, Status> {
        let RemoveTargetRequest {
            machine_id,
            target_id,
        } = req.into_inner();
        let target_id =
            Uuid::parse_str(&target_id).map_err(|e| Status::invalid_argument(e.to_string()))?;
        self.sessions
            .write()
            .remove_target(machine_id.into(), target_id)
            .map_err(|e| match e {
                SessionError::TargetNotFound { .. } => Status::not_found(e.to_string()),
                _ => Status::failed_precondition(e.to_string()),
            })?;
        Ok(Response::new(RemoveTargetResponse {}))
    }

    #[instrument]
    async fn hit_target(
        &self,
        req: Request<HitTargetRequest>,
    ) -> Result<Response<HitTargetResponse>, Status> {
        let HitTargetRequest {
            machine_id,
            target_id,
        } = req.into_inner();
        let target_id =
            Uuid::parse_str(&target_id).map_err(|e| Status::invalid_argument(e.to_string()))?;
        let (total_score, ev_target) = self
            .sessions
            .write()
            .hit_target(machine_id.into(), target_id)
            .map_err(|e| match e {
                HitTargetError::Session(SessionError::TargetNotFound { .. }) => {
                    Status::not_found(e.to_string())
                }
                HitTargetError::ScoreOverflow => Status::out_of_range(e.to_string()),
                _ => Status::failed_precondition(e.to_string()),
            })?;
        if self
            .event_tx
            .send(Event {
                target: ev_target,
                data: Ok(SuccessfulEvent::GameScoreChanged { total_score }),
            })
            .is_err()
        {
            warn!("all event receiver has been dropped");
        }
        Ok(Response::new(HitTargetResponse {}))
    }

    #[instrument]
    async fn get_game_result(
        &self,
        req: Request<GetGameResultRequest>,
    ) -> Result<Response<GetGameResultResponse>, Status> {
        let GetGameResultRequest { machine_id } = req.into_inner();
        let total_score = self
            .sessions
            .read()
            .game_result(machine_id.into())
            .map_err(|e| Status::failed_precondition(e.to_string()))?
            .score;
        Ok(Response::new(GetGameResultResponse { total_score }))
    }

    #[instrument]
    async fn complete_session(
        &self,
        req: Request<CompleteSessionRequest>,
    ) -> Result<Response<CompleteSessionResponse>, Status> {
        let CompleteSessionRequest { machine_id } = req.into_inner();
        let game = self
            .sessions
            .write()
            .complete_session(machine_id.into())
            .map_err(|e| Status::failed_precondition(e.to_string()))?;
        if let Some(game) = game {
            self.data_source
                .save_game(game)
                .await
                .map_err(|e| Status::internal(e.to_string()))?;
        }
        Ok(Response::new(CompleteSessionResponse {}))
    }

    #[instrument]
    async fn set_session_ready(
        &self,
        req: Request<SetSessionReadyRequest>,
    ) -> Result<Response<SetSessionReadyResponse>, Status> {
        let SetSessionReadyRequest { machine_id } = req.into_inner();
        let machine_id = machine_id.into();
        {
            let action = {
                let mut sessions = self.sessions.write();
                sessions.set_ready(machine_id)
            };
            match action {
                Ok(SetReadyAction::RunSingle { difficulty }) => {
                    if self
                        .event_tx
                        .send(Event {
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
                        self.event_tx.clone(),
                        complete_tx,
                    ));

                    // handle game completion
                    tokio::spawn(
                        self.clone()
                            .handle_game_completion(EventTarget::Single(machine_id), complete_rx),
                    );
                    Ok(Response::new(SetSessionReadyResponse {}))
                }
                Ok(SetReadyAction::RunMulti { difficulty }) => {
                    if self
                        .event_tx
                        .send(Event {
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
                        self.event_tx.clone(),
                        complete_tx,
                    ));

                    // handle game completion
                    tokio::spawn(
                        self.clone()
                            .handle_game_completion(EventTarget::All, complete_rx),
                    );
                    Ok(Response::new(SetSessionReadyResponse {}))
                }
                Ok(SetReadyAction::WaitForAnother) => Ok(Response::new(SetSessionReadyResponse {})),
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
    tx: broadcast::Sender<Event>,
    complete_tx: mpsc::Sender<()>,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    let duration_secs = GAME_DURATION.whole_seconds();
    for i in 0..duration_secs {
        trace!(secs_elapsed = i, "timer tick");
        let res = tx.send(Event {
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
    /// [`struckout_proto::event::SessionDifficultyChanged`]
    SessionDifficultyChanged(Difficulty),
    /// [`struckout_proto::event::TutorialStarted`]
    TutorialStarted,
    /// [`struckout_proto::event::TutorialNavigatedToPrevious`]
    TutorialNavigatedToPrevious,
    /// [`struckout_proto::event::TutorialNavigatedToNext`]
    TutotialNavigatedToNext,
}

impl From<SuccessfulEvent> for struckout_proto::event::EventData {
    fn from(ev: SuccessfulEvent) -> Self {
        use struckout_proto::event::*;
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
            SuccessfulEvent::SessionDifficultyChanged(difficulty) => {
                EventData::SessionDifficultyChanged(SessionDifficultyChanged {
                    difficulty: difficulty.into(),
                })
            }
            SuccessfulEvent::TutorialStarted => EventData::TutorialStarted(TutorialStarted {}),
            SuccessfulEvent::TutorialNavigatedToPrevious => {
                EventData::TutorialNavigatedToPrevious(TutorialNavigatedToPrevious {})
            }
            SuccessfulEvent::TutotialNavigatedToNext => {
                EventData::TutorialNavigatedToNext(TutorialNavigatedToNext {})
            }
        }
    }
}
