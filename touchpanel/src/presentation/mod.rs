use std::{sync::Arc, time::Duration};

use stern::{WorkerThread, nav::NavHost, worker::ForegroundExecutor};
use struckout_proto::{GameMode, types::PlayerId};
use tokio::{sync::oneshot, time::timeout};
use tonic::Status;
use touchpanel_ui::{MultiPlayAvailability, NavRoute};
use tracing::{debug, warn};

use crate::{
    Application, Config, Context, NavController,
    data::{GameMasterClient, SessionPhase},
    presentation::{
        connecting::ConnectingDestination, connection_failed::ConnectionFailedDestination,
        difficulty_select::DifficultySelectDestination, fallback::FallbackDestination,
        game_finished::GameFinishedDestination, mode_select::ModeSelectDestination,
        name_input_first::NameInputFirstDestination,
        name_input_non_first::NameInputNonFirstDestination, playing::PlayingDestination,
        ranking_control::RankingControlDestination, start::StartScreenDestination,
    },
};

/// Defines `XxxViewModelRc` which wraps `XxxViewModel`.
///
/// `XxxViewModelRc::new()` registers viewmodel to the adopter
/// by calling [`stern::GlobalExt::register_viewmodel()`]).
///
/// `vm` is the name of viewmodel type (e.g. XxxViewModel).
///
/// # Example
/// ```ignore
/// # use crate::Context;
/// # use stern::worker::SlintExecutor;
/// use touchpanel_ui::{StartViewModel, StartAdopter};
///
/// viewmodel_rc!(StartViewModel<Context,SlintExecutor,>, StartAdopter);
/// ```
macro_rules! viewmodel_rc {
    ($vm:ident$(<$type_param1:ty$(,$type_param2:ty)*>)?, $adopter:ty) => {
        pastey::paste! {
            #[derive(derive_more::Deref)]
            struct [<$vm Rc>](std::rc::Rc<std::cell::RefCell<$vm$(<$type_param1$(,$type_param2)*>)?>>);

            impl [<$vm Rc>] {
                #[doc = concat!("Creates [`", stringify!($vm), "`] and registers it to the adapter by calling [`stern::GlobalExt::register_viewmodel()`].")]
                fn new(application: &Application) -> Self {
                    use slint::ComponentHandle as _;
                    use stern::GlobalExt as _;

                    let this = std::rc::Rc::new(std::cell::RefCell::new($vm::new(application)));
                    application.ui.global::<touchpanel_ui::$adopter>()
                        .register_viewmodel(std::rc::Rc::clone(&this));

                    Self(this)
                }
            }
        }
    };
}

pub mod connecting;
pub mod connection_failed;
pub mod difficulty_select;
pub mod fallback;
pub mod game_finished;
pub mod mode_select;
pub mod name_input_first;
pub mod name_input_non_first;
pub mod playing;
pub mod ranking_control;
pub mod start;
//@xtask-pub-mod

pub(crate) trait SingleSessionStarter: Send + Sync + 'static {
    fn start_single_session(
        &self,
        player_id: PlayerId,
    ) -> impl Future<Output = Result<(), Status>> + Send;
}

impl SingleSessionStarter for Context {
    async fn start_single_session(&self, player_id: PlayerId) -> Result<(), Status> {
        let mut gm = self
            .game_master
            .get()
            .expect("connection is initialized")
            .clone();
        gm.start_session(player_id).await?;
        gm.set_game_mode(GameMode::Single).await
    }
}

async fn route_after_player<C: SingleSessionStarter>(
    context: &C,
    multiplay_availability: MultiPlayAvailability,
    player_id: PlayerId,
) -> Result<NavRoute, Status> {
    if multiplay_availability == MultiPlayAvailability::Disabled {
        context.start_single_session(player_id).await?;
        Ok(NavRoute::DifficulitySelect)
    } else {
        Ok(NavRoute::ModeSelect { player_id })
    }
}

/// Connects to game-master's gRPC server. **Must be called within tokio context**.
///
/// When succeeded, it returns a client with its event subscription already active.
/// When failed, it navigates to `ConnectionFailedScreen` with error message.
async fn connect_to_game_master<E>(
    worker: &WorkerThread<Context, E>,
    nc: NavController,
    config: Arc<Config>,
) -> Result<GameMasterClient, ()>
where
    E: ForegroundExecutor,
{
    let (tx, rx) = oneshot::channel();

    worker.spawn_cx({
        async move |_| {
            let res = timeout(
                Duration::from_secs(5),
                GameMasterClient::connect(config.server_addr.as_ref(), config.machine_id.into()),
            )
            .await;
            tx.send(res).unwrap();
        }
    });
    match rx.await.expect("channel should not be closed") {
        Ok(Ok(v)) => Ok(v),
        Ok(Err(e)) => {
            nc.navigate(NavRoute::ConnectionFailed(format!(
                "game-masterへの接続に失敗しました: {}",
                e
            )));
            Err(())
        }
        Err(_) => {
            nc.navigate(NavRoute::ConnectionFailed(
                "タイムアウトしました".to_string(),
            ));
            Err(())
        }
    }
}

/// Initializes [`WorkerThread`]'s context. Note that since the function calls [`slint::spawn_local()`] to await async functions,
/// the process actually starts after slint's event loop has started. (e.g. by [`AppWindow::run()`])
///
/// [AppWindow::run]: crate::ui::AppWindow::run
pub fn init_worker_context(application: &Application) {
    let nc = application.nav_controller.clone();
    let worker = application.worker.clone();
    let config = application.config.clone();
    slint::spawn_local(async move {
        debug!("initializing worker context");
        let game_master = match connect_to_game_master(&worker, nc.clone(), config).await {
            Ok(v) => v,
            Err(_) => {
                warn!("failed to connect to game-master. user can retry it.");
                return;
            }
        };

        {
            let cx = worker.background_executor().context();
            cx.game_master
                .set(game_master)
                .expect("this should be a first successful attempt to connect to game-master");
        }
        observe_session(&worker, nc.clone());
        nc.navigate(NavRoute::Start);
        debug!("worker context initialized successfully");
    })
    .unwrap();
}

/// Observes the retained snapshot so start and finish events cannot be lost during navigation.
fn observe_session<E: ForegroundExecutor>(worker: &WorkerThread<Context, E>, nc: NavController) {
    let mut session = worker
        .background_executor()
        .context()
        .game_master
        .get()
        .expect("connection is initialized")
        .session();
    worker.spawn_local(async move {
        let mut previous_phase = SessionPhase::Idle;
        loop {
            let snapshot = session.borrow_and_update().clone();
            if let Some(error) = snapshot.error {
                nc.navigate(NavRoute::Fallback(format!(
                    "game-masterとの通信に失敗しました: {error}"
                )));
                break;
            }
            if snapshot.phase != previous_phase {
                match snapshot.phase {
                    SessionPhase::Running => nc.navigate(NavRoute::Playing),
                    SessionPhase::Finished(_) => nc.navigate(NavRoute::GameFinished),
                    _ => {}
                }
                previous_phase = snapshot.phase;
            }
            if session.changed().await.is_err() {
                break;
            }
        }
    });
}

/// Registers each [`NavDestination`][crate::nav::NavDestination]s at [`NavHost`].
pub fn attach_navhost(application: &Application) {
    NavHost::builder(application.nav_controller.clone())
        .register(StartScreenDestination::new(application))
        .register(NameInputFirstDestination::new(application))
        .register(ModeSelectDestination::new(application))
        .register(DifficultySelectDestination::new(application))
        .register(FallbackDestination::new(application))
        .register(ConnectionFailedDestination::new(application))
        .register(PlayingDestination::new(application))
        .register(GameFinishedDestination::new(application))
        .register(ConnectingDestination::new(application))
        .register(RankingControlDestination::new(application))
        .register(NameInputNonFirstDestination::new(application))
        //@xtask-register
        .finish()
        .expect("failed to build NavHost");
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use slint::ComponentHandle as _;
    use stern::nav::NavController;

    use super::*;

    struct TestSessionStarter {
        starts: AtomicUsize,
        fail: bool,
    }

    impl SingleSessionStarter for TestSessionStarter {
        async fn start_single_session(&self, _player_id: PlayerId) -> Result<(), Status> {
            self.starts.fetch_add(1, Ordering::Relaxed);
            if self.fail {
                Err(Status::unavailable("session failed"))
            } else {
                Ok(())
            }
        }
    }

    #[tokio::test]
    async fn disabled_multiplay_starts_single_session_before_difficulty_selection() {
        let starter = TestSessionStarter {
            starts: AtomicUsize::new(0),
            fail: false,
        };
        let route = route_after_player(&starter, MultiPlayAvailability::Disabled, 1.into())
            .await
            .unwrap();

        assert!(matches!(route, NavRoute::DifficulitySelect));
        assert_eq!(starter.starts.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn enabled_multiplay_shows_mode_selection_without_starting_session() {
        let starter = TestSessionStarter {
            starts: AtomicUsize::new(0),
            fail: false,
        };
        let route = route_after_player(&starter, MultiPlayAvailability::Enabled, 1.into())
            .await
            .unwrap();

        assert!(matches!(route, NavRoute::ModeSelect { .. }));
        assert_eq!(starter.starts.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn disabled_multiplay_does_not_navigate_when_session_start_fails() {
        let starter = TestSessionStarter {
            starts: AtomicUsize::new(0),
            fail: true,
        };
        let result = route_after_player(&starter, MultiPlayAvailability::Disabled, 1.into()).await;

        assert!(result.is_err());
        assert_eq!(starter.starts.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn attach_navhost_registers_all_screen() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = touchpanel_ui::AppWindow::new().unwrap();

        let nav_controller = NavController::new(NavRoute::Connecting, {
            let ui = ui.as_weak();

            move |route| {
                let ui = ui.unwrap();
                ui.set_nav_route(route);
            }
        });

        let cx = Context::new_empty();
        let worker = WorkerThread::new(cx);

        let application = Application {
            nav_controller,
            ui,
            worker,
            config: Arc::new(Config {
                server_addr: "127.0.0.1".to_string(),
                machine_id: 1,
                multiplay_availability: touchpanel_ui::MultiPlayAvailability::Enabled,
                monitor: None,
            }),
        };

        attach_navhost(&application);
    }
}
