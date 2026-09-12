use crate::{
    Application, Context, NavController,
    ui::{self, NavRoute, NavRouteKind, PlayingStates, PlayingViewModelTrait},
};
use slint::{ComponentHandle, Global, ToSharedString};
use stern::{WorkerThread, nav::NavDestination};
use tokio_stream::{StreamExt as _, wrappers::WatchStream};
use tracing::debug;

viewmodel_rc!(PlayingViewModel, PlayingAdopter);

struct PlayingViewModel {
    nav_controller: NavController,
    worker: WorkerThread<Context>,
    state: PlayingStates,
}

impl PlayingViewModel {
    fn new(application: &Application) -> Self {
        Self {
            nav_controller: application.nav_controller.clone(),
            worker: application.worker.clone(),
            state: PlayingStates::new(application.ui.global::<ui::PlayingAdopter>().as_weak()),
        }
    }

    /// Start listening states of the current session.
    fn listen_session(&self) {
        let mut worker = self.worker.clone();

        let (rem_rx, score_rx, mut error_rx) = {
            let cx = worker.context();
            let cx_guard = cx.read();
            let session_guard = cx_guard.game_master.get().unwrap().session();
            let session = session_guard.as_ref().unwrap();

            (session.remaining_time(), session.score(), session.error())
        };
        let rem_stream = WatchStream::new(rem_rx)
            .map(|v| v.to_shared_string())
            .fuse();
        let rem_prop = self.state.remaining_time.clone();
        let rem_join = rem_prop.bind_detached(rem_stream);

        let score_stream = WatchStream::new(score_rx)
            .map(|v| v.try_into().expect("score overflowed"))
            .fuse();
        let score_prop = self.state.score.clone();
        let score_join = score_prop.bind_detached(score_stream);

        slint::spawn_local({
            let nc = self.nav_controller.clone();
            async move {
                loop {
                    error_rx.changed().await.unwrap();
                    if let Some(err) = error_rx.borrow_and_update().as_ref() {
                        nc.navigate(NavRoute::Fallback(err.to_string()));
                    }
                }
            }
        })
        .unwrap();
    }
}

impl PlayingViewModelTrait for PlayingViewModel {}

pub struct PlayingDestination(
    #[allow(dead_code)] // may used when some arg is added to the route
    PlayingViewModelRc,
);

impl PlayingDestination {
    pub fn new(application: &Application) -> Self {
        Self(PlayingViewModelRc::new(application))
    }
}

impl NavDestination<NavRoute> for PlayingDestination {
    fn load(&self, route: &NavRoute) {
        debug!("loading PlayingViewModel");

        let NavRoute::Playing(difficulty) = route else {
            panic!("matched variant should be given");
        };

        self.0.borrow().listen_session();
    }

    fn route(&self) -> NavRouteKind {
        NavRouteKind::Playing
    }
}
