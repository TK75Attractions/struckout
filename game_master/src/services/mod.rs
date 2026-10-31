mod conversion;
use conversion::{ScoreReason, SuccessfulEvent};
mod player;
pub use player::GameMasterPlayerServiceImpl;
#[cfg(test)]
mod tests;

use std::{num::NonZeroU32, pin::Pin, sync::Arc, time::Duration};

use parking_lot::RwLock;
use serde::Deserialize;
use struckout_proto::{
    self, AddTargetRequest, AddTargetResponse, Difficulty, GameMode, GetGameResultRequest,
    GetGameResultResponse, GetGamesByPlayerRequest, GetGamesByPlayerResponse, GetRankingRequest,
    GetRankingResponse, GoHomeScreenRequest, GoHomeScreenResponse, GoNextTutorialRequest,
    GoNextTutorialResponse, GoPreviousTutorialRequest, GoPreviousTutorialResponse,
    GoRankingScreenRequest, GoRankingScreenResponse, HitTargetRequest, HitTargetResponse,
    ListenEventsRequest, ListenEventsResponse, RemoveTargetRequest, RemoveTargetResponse,
    ResetComboRequest, ResetComboResponse, SetGameModeRequest, SetGameModeResponse,
    SetSessionDifficultyRequest, SetSessionDifficultyResponse, SetSessionReadyRequest,
    SetSessionReadyResponse, StartSessionRequest, StartSessionResponse, StartTutorialRequest,
    StartTutorialResponse,
    game_master_session_service_server::GameMasterSessionService,
    target::Owner,
    types::{MachineId, PlayerId},
};
use time::ext::NumericalDuration;
use tokio::sync::{broadcast, mpsc};
use tokio_stream::{
    Stream, StreamExt,
    wrappers::{BroadcastStream, ReceiverStream},
};
use tonic::{Request, Response, Status};
use tracing::{instrument, trace, warn};
use uuid::Uuid;

use crate::{
    DataSource, DataSourceImpl, GamePlayerInfo, GetGameResultError, PlayerInfo,
    session::{ComboBonuses, HitTargetError, SessionError, Sessions, SetReadyAction},
};

/// Implementation of [`GameMasterSessionService`].
#[derive(derive_more::Debug, Clone)]
pub struct GameMasterSessionServiceImpl<D = DataSourceImpl> {
    sessions: Arc<RwLock<Sessions>>,
    #[debug(skip)]
    data_source: D,
    event_tx: broadcast::Sender<Event>,
    game_start_delay_secs: NonZeroU32,
    game_duration_secs: GameDurations,
    combo_bonuses: ComboBonuses,
}

/// Holds every selectable difficulty's duration so game start never needs a fallback.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameDurations {
    pub normal: NonZeroU32,
    pub hard: NonZeroU32,
    pub veryhard: NonZeroU32,
}

impl GameDurations {
    fn for_difficulty(self, difficulty: Difficulty) -> NonZeroU32 {
        match difficulty {
            Difficulty::Normal => self.normal,
            Difficulty::Hard => self.hard,
            Difficulty::Veryhard => self.veryhard,
            Difficulty::Unspecified => {
                // set_session_difficulty rejects unspecified before a game can start.
                unreachable!("a running game must have a selectable difficulty")
            }
        }
    }
}

impl<D> GameMasterSessionServiceImpl<D>
where
    D: DataSource,
{
    pub fn new(
        data_source: D,
        game_start_delay_secs: NonZeroU32,
        game_duration_secs: GameDurations,
        combo_bonuses: ComboBonuses,
    ) -> Self {
        let (event_tx, _) = broadcast::channel(32);
        Self {
            sessions: Default::default(),
            data_source,
            event_tx,
            game_start_delay_secs,
            game_duration_secs,
            combo_bonuses,
        }
    }

    /// Handles completion of game when notified via `complete_rx`.
    async fn handle_game_completion(
        self,
        ev_target: EventTarget,
        mut complete_rx: mpsc::Receiver<()>,
    ) {
        complete_rx.recv().await.expect("complete channel closed");
        let (machine_id, game) = {
            let mut sessions = self.sessions.write();
            let machine_id = match ev_target {
                EventTarget::Single(machine_id) => machine_id,
                EventTarget::All => sessions.machine_ids().expect("multi-play has two sessions")[0],
            };
            sessions
                .finish_game(machine_id)
                .expect("only running games have a completion timer");
            let game = sessions
                .game_result(machine_id)
                .expect("game was finished above")
                .clone();
            (machine_id, game)
        };
        let data = self
            .data_source
            .save_game(game)
            .await
            .map(|game_id| SuccessfulEvent::GameFinished { game_id })
            .map_err(|e| Status::internal(e.to_string()));
        self.sessions
            .write()
            .discard_game(machine_id)
            .expect("the finished game is retained during database I/O");
        if self
            .event_tx
            .send(Event {
                target: ev_target,
                data,
            })
            .is_err()
        {
            warn!("all event receivers have been dropped");
        }
    }
}

/// Gets [`EventTarget`] from the given [`Target`][struckout_proto::Target].
fn get_event_target(target: &struckout_proto::Target) -> EventTarget {
    // Sessions::add_target always registers an owner with each target.
    match target
        .owner
        .as_ref()
        .expect("registered target has an owner")
    {
        Owner::Single(owner) => EventTarget::Single(owner.machine_id.into()),
        Owner::All(_) => EventTarget::All,
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
        if difficulty == Difficulty::Unspecified {
            return Err(Status::invalid_argument("difficulty must be specified"));
        }
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
        let mut sessions = self.sessions.write();
        let (target_id, target) = sessions
            .add_target(
                target_data,
                Owner::Single(struckout_proto::target::OwnerSingle { machine_id }),
            )
            .map_err(|e| Status::failed_precondition(e.to_string()))?;
        if self
            .event_tx
            .send(Event {
                target: get_event_target(&target),
                data: Ok(SuccessfulEvent::TargetAdded { target }),
            })
            .is_err()
        {
            warn!("all event receivers have been dropped");
        }
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
        let target = {
            let mut sessions = self.sessions.write();
            sessions
                .remove_target(machine_id.into(), target_id)
                .map_err(|e| match e {
                    SessionError::TargetNotFound { .. } => Status::not_found(e.to_string()),
                    _ => Status::failed_precondition(e.to_string()),
                })?
        };
        if self
            .event_tx
            .send(Event {
                target: get_event_target(&target),
                data: Ok(SuccessfulEvent::TargetRemoved {
                    target_id: target.target_id,
                }),
            })
            .is_err()
        {
            warn!("all event receivers have been dropped");
        }
        Ok(Response::new(RemoveTargetResponse {}))
    }

    #[instrument]
    async fn reset_combo(
        &self,
        req: Request<ResetComboRequest>,
    ) -> Result<Response<ResetComboResponse>, Status> {
        let machine_id = req.into_inner().machine_id.into();
        let mut sessions = self.sessions.write();
        if let Some(target) = sessions
            .reset_combo(machine_id)
            .map_err(|e| Status::failed_precondition(e.to_string()))?
            && self
                .event_tx
                .send(Event {
                    target,
                    data: Ok(SuccessfulEvent::ComboChanged { combo: 0 }),
                })
                .is_err()
        {
            warn!("all event receivers have been dropped");
        }
        Ok(Response::new(ResetComboResponse {}))
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
        let mut sessions = self.sessions.write();
        let outcome = sessions
            .hit_target(machine_id.into(), target_id, &self.combo_bonuses)
            .map_err(|e| match e {
                HitTargetError::Session(SessionError::TargetNotFound { .. }) => {
                    Status::not_found(e.to_string())
                }
                HitTargetError::ScoreOverflow => Status::out_of_range(e.to_string()),
                _ => Status::failed_precondition(e.to_string()),
            })?;
        let score_events = [
            SuccessfulEvent::GameScoreChanged {
                total_score: outcome.total_score - outcome.combo_bonus,
                diff: outcome.target_points,
                reason: ScoreReason::TargetHit {
                    target_id: target_id.to_string(),
                },
            },
            SuccessfulEvent::GameScoreChanged {
                total_score: outcome.total_score,
                diff: outcome.combo_bonus,
                reason: ScoreReason::Combo,
            },
        ];
        for event in score_events
            .into_iter()
            .take(if outcome.combo_bonus > 0 { 2 } else { 1 })
        {
            if self
                .event_tx
                .send(Event {
                    target: outcome.event_target,
                    data: Ok(event),
                })
                .is_err()
            {
                warn!("all event receiver has been dropped");
            }
        }
        if self
            .event_tx
            .send(Event {
                target: outcome.event_target,
                data: Ok(SuccessfulEvent::ComboChanged {
                    combo: outcome.combo,
                }),
            })
            .is_err()
        {
            warn!("all event receivers have been dropped");
        }
        if let Some((removed, added)) = outcome.replacement {
            for event in [
                Event {
                    target: get_event_target(&removed),
                    data: Ok(SuccessfulEvent::TargetRemoved {
                        target_id: removed.target_id,
                    }),
                },
                Event {
                    target: get_event_target(&added),
                    data: Ok(SuccessfulEvent::TargetAdded { target: added }),
                },
            ] {
                if self.event_tx.send(event).is_err() {
                    warn!("all event receivers have been dropped");
                }
            }
        }
        Ok(Response::new(HitTargetResponse {}))
    }

    #[instrument]
    async fn get_game_result(
        &self,
        req: Request<GetGameResultRequest>,
    ) -> Result<Response<GetGameResultResponse>, Status> {
        let GetGameResultRequest { game_id } = req.into_inner();
        let total_score = self
            .data_source
            .get_game_result(game_id.into())
            .await
            .map_err(|e| match e {
                GetGameResultError::NotFound(_) => Status::not_found(e.to_string()),
                GetGameResultError::Sqlx(_) => Status::internal(e.to_string()),
            })?;
        Ok(Response::new(GetGameResultResponse { total_score }))
    }

    #[instrument]
    async fn set_session_ready(
        &self,
        req: Request<SetSessionReadyRequest>,
    ) -> Result<Response<SetSessionReadyResponse>, Status> {
        let countdown_started_at = tokio::time::Instant::now();
        let SetSessionReadyRequest { machine_id } = req.into_inner();
        let machine_id = machine_id.into();
        // Resolve names before changing state so a database failure cannot leave a game running without notifications.
        let (configuration, partner) = {
            let sessions = self.sessions.read();
            let session = sessions
                .get(machine_id)
                .ok_or_else(|| Status::failed_precondition("session does not exist"))?;
            let (state,) = session.as_configuring_difficulty().map_err(|_| {
                Status::failed_precondition("session is not configuring difficulty")
            })?;
            if state.difficulty.is_none() {
                return Err(Status::failed_precondition("difficulty is required"));
            }
            let partner = if state.mode == GameMode::Multi {
                sessions
                    .get_another(machine_id)
                    .and_then(|other| other.as_waiting_for_another_machine().ok())
                    .map(|(state,)| state.clone())
            } else {
                None
            };
            (state.clone(), partner)
        };
        let first = self
            .data_source
            .get_player_by_id(configuration.player_id)
            .await
            .map_err(|error| Status::internal(error.to_string()))?;
        let second = if let Some(partner) = &partner {
            Some(
                self.data_source
                    .get_player_by_id(partner.player_id)
                    .await
                    .map_err(|error| Status::internal(error.to_string()))?,
            )
        } else {
            None
        };

        {
            let mut sessions = self.sessions.write();
            let current_configuration = sessions
                .get(machine_id)
                .and_then(|session| session.as_configuring_difficulty().ok())
                .map(|(state,)| state);
            let current_partner = if configuration.mode == GameMode::Multi {
                sessions
                    .get_another(machine_id)
                    .and_then(|other| other.as_waiting_for_another_machine().ok())
                    .map(|(state,)| state)
            } else {
                None
            };
            if current_configuration != Some(&configuration) || current_partner != partner.as_ref()
            {
                return Err(Status::aborted(
                    "session configuration changed while loading players",
                ));
            }
            let action = sessions.set_ready(machine_id);
            match action {
                Ok(SetReadyAction::RunSingle {
                    initial_targets,
                    difficulty,
                    player_id,
                }) => {
                    let game_duration_secs = self.game_duration_secs.for_difficulty(difficulty);
                    let (complete_tx, complete_rx) = mpsc::channel(16);
                    tokio::spawn(countdown_and_start(
                        CountdownStart {
                            ev_target: EventTarget::Single(machine_id),
                            started_at: countdown_started_at,
                            delay_secs: self.game_start_delay_secs,
                            started: SuccessfulEvent::GameStarted {
                                game_duration_secs: game_duration_secs.get(),
                                difficulty,
                                players: GamePlayerInfo::Single(PlayerInfo {
                                    machine_id,
                                    player_id,
                                    player_name: first.name,
                                }),
                            },
                            initial_targets,
                            game_duration_secs,
                            sessions: self.sessions.clone(),
                            machine_id,
                        },
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
                Ok(SetReadyAction::RunMulti {
                    initial_targets,
                    difficulty,
                    player_1,
                    player_2,
                }) => {
                    let game_duration_secs = self.game_duration_secs.for_difficulty(difficulty);
                    let (complete_tx, complete_rx) = mpsc::channel(16);
                    tokio::spawn(countdown_and_start(
                        CountdownStart {
                            ev_target: EventTarget::All,
                            started_at: countdown_started_at,
                            delay_secs: self.game_start_delay_secs,
                            started: SuccessfulEvent::GameStarted {
                                game_duration_secs: game_duration_secs.get(),
                                difficulty,
                                players: GamePlayerInfo::Multi(
                                    PlayerInfo {
                                        machine_id: player_1.0,
                                        player_id: player_1.1,
                                        player_name: first.name,
                                    },
                                    PlayerInfo {
                                        machine_id: player_2.0,
                                        player_id: player_2.1,
                                        player_name: second
                                            .expect(
                                                "partner was resolved before starting multi-play",
                                            )
                                            .name,
                                    },
                                ),
                            },
                            initial_targets,
                            game_duration_secs,
                            sessions: self.sessions.clone(),
                            machine_id: player_1.0,
                        },
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

    #[instrument]
    async fn get_ranking(
        &self,
        req: Request<GetRankingRequest>,
    ) -> Result<Response<GetRankingResponse>, Status> {
        use struckout_proto::get_ranking_response;
        let req = req.into_inner();
        let (mode, difficulty) = validate_result_filter(req.mode, req.difficulty)?;
        let items = self
            .data_source
            .get_ranking(req.max_item_num, mode, difficulty)
            .await
            .map_err(|error| Status::internal(error.to_string()))?;
        Ok(Response::new(GetRankingResponse {
            top_n: items
                .into_iter()
                .map(|item| get_ranking_response::RankingItem {
                    game_id: item.result.game_id.into(),
                    total_score: item.result.total_score,
                    player_info: Some(item.players.into()),
                })
                .collect(),
        }))
    }

    #[instrument]
    async fn get_games_by_player(
        &self,
        req: Request<GetGamesByPlayerRequest>,
    ) -> Result<Response<GetGamesByPlayerResponse>, Status> {
        let req = req.into_inner();
        let (mode, difficulty) = validate_result_filter(req.mode, req.difficulty)?;
        let items = self
            .data_source
            .get_games_by_player(req.player_id.into(), mode, difficulty)
            .await
            .map_err(|error| Status::internal(error.to_string()))?;
        Ok(Response::new(GetGamesByPlayerResponse {
            items: items
                .into_iter()
                .map(
                    |item| struckout_proto::get_games_by_player_response::GamesByPlayerItem {
                        game_id: item.item.result.game_id.into(),
                        total_score: item.item.result.total_score,
                        player_info: Some(item.item.players.into()),
                        rank: item.rank,
                    },
                )
                .collect(),
        }))
    }

    #[instrument]
    async fn go_ranking_screen(
        &self,
        req: Request<GoRankingScreenRequest>,
    ) -> Result<Response<GoRankingScreenResponse>, Status> {
        let machine_id = MachineId::new(req.into_inner().machine_id);
        self.event_tx
            .send(Event {
                target: EventTarget::Single(machine_id),
                data: Ok(SuccessfulEvent::NavigateToRankingScreenRequested(
                    machine_id,
                )),
            })
            .map_err(|_| Status::unavailable("no event subscribers"))?;
        Ok(Response::new(GoRankingScreenResponse {}))
    }

    #[instrument]
    async fn go_home_screen(
        &self,
        req: Request<GoHomeScreenRequest>,
    ) -> Result<Response<GoHomeScreenResponse>, Status> {
        let machine_id = MachineId::new(req.into_inner().machine_id);
        self.event_tx
            .send(Event {
                target: EventTarget::Single(machine_id),
                data: Ok(SuccessfulEvent::NavigateToHomeScreenRequested(machine_id)),
            })
            .map_err(|_| Status::unavailable("no event subscribers"))?;
        Ok(Response::new(GoHomeScreenResponse {}))
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

struct CountdownStart {
    ev_target: EventTarget,
    started_at: tokio::time::Instant,
    delay_secs: NonZeroU32,
    started: SuccessfulEvent,
    initial_targets: Vec<struckout_proto::Target>,
    game_duration_secs: NonZeroU32,
    sessions: Arc<RwLock<Sessions>>,
    machine_id: MachineId,
}

/// Sends the countdown, starts play, then runs the game timer.
#[instrument(skip(start, tx, complete_tx))]
async fn countdown_and_start(
    start: CountdownStart,
    tx: broadcast::Sender<Event>,
    complete_tx: mpsc::Sender<()>,
) {
    let CountdownStart {
        ev_target,
        started_at,
        delay_secs,
        started,
        initial_targets,
        game_duration_secs,
        sessions,
        machine_id,
    } = start;
    let mut interval = tokio::time::interval_at(started_at, Duration::from_secs(1));
    for remaining in (1..=delay_secs.get()).rev() {
        interval.tick().await;
        if tx
            .send(Event {
                target: ev_target,
                data: Ok(SuccessfulEvent::GameWillStart {
                    secs_until_start: remaining,
                }),
            })
            .is_err()
        {
            warn!("all event receivers have been dropped");
        }
    }
    interval.tick().await;
    sessions.write().mark_started(machine_id);
    if tx
        .send(Event {
            target: ev_target,
            data: Ok(started),
        })
        .is_err()
    {
        warn!("all event receivers have been dropped");
    }
    for target in initial_targets {
        if tx
            .send(Event {
                target: get_event_target(&target),
                data: Ok(SuccessfulEvent::TargetAdded { target }),
            })
            .is_err()
        {
            warn!("all event receivers have been dropped");
        }
    }
    game_timer(ev_target, game_duration_secs, tx, complete_tx).await;
}

#[instrument(skip(tx, complete_tx))]
async fn game_timer(
    ev_target: EventTarget,
    game_duration_secs: NonZeroU32,
    tx: broadcast::Sender<Event>,
    complete_tx: mpsc::Sender<()>,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    interval.tick().await;
    let duration_secs = i64::from(game_duration_secs.get());
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

/// Validates the required mode and difficulty of result queries.
fn validate_result_filter(mode: i32, difficulty: i32) -> Result<(GameMode, Difficulty), Status> {
    let mode =
        GameMode::try_from(mode).map_err(|error| Status::invalid_argument(error.to_string()))?;
    let difficulty = Difficulty::try_from(difficulty)
        .map_err(|error| Status::invalid_argument(error.to_string()))?;
    if mode == GameMode::Unspecified || difficulty == Difficulty::Unspecified {
        return Err(Status::invalid_argument(
            "mode and difficulty must be specified",
        ));
    }
    Ok((mode, difficulty))
}
