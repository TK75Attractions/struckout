use crate::{
    Application, Context, NavController,
    data::{DisplayableRemainingTime, Session},
};
use parking_lot::RwLockReadGuard;
use slint::{ComponentHandle, Global, ToSharedString};
use stern::{WorkerThread, nav::NavDestination};
use struckout_proto::types::GameId;
use tokio_stream::{StreamExt as _, wrappers::WatchStream};
use touchpanel_ui::{
    NavRoute, NavRouteKind, PlayingPropertyMappers, PlayingStates, PlayingViewModelTrait,
};
use tracing::debug;

viewmodel_rc!(PlayingViewModel<C>, PlayingAdopter);

pub struct PlayingViewModel<C> {
    nav_controller: NavController,
    worker: WorkerThread<C>,
    state: PlayingStates<Mapper>,
}

touchpanel_ui::define_playing_mapper! {
    remaining_time to DisplayableRemainingTime {
        |value: DisplayableRemainingTime| value.to_shared_string()
    },
    score to u32 {
        |value: u32| value.try_into().expect("failed to convert u32 into i32")
    },
}

pub trait SessionProvider {
    fn session(&self) -> RwLockReadGuard<'_, Option<Session>>;
}

impl SessionProvider for Context {
    /// Panics when [`Context`] is not initialized.
    fn session(&self) -> RwLockReadGuard<'_, Option<Session>> {
        self.game_master.get().unwrap().session()
    }
}

impl PlayingViewModel<Context> {
    fn new(application: &Application) -> Self {
        Self {
            nav_controller: application.nav_controller.clone(),
            worker: application.worker.clone(),
            state: PlayingStates::<Mapper>::new(
                application
                    .ui
                    .global::<touchpanel_ui::PlayingAdopter>()
                    .as_weak(),
            ),
        }
    }
}

impl<C> PlayingViewModel<C>
where
    C: SessionProvider + 'static,
{
    /// Start listening states of the current session.
    ///
    /// Returns [`slint::JoinHandle`] which completes after cleaning all properties.
    pub fn listen_session(&self, game_id: GameId) -> slint::JoinHandle<()> {
        let mut worker = self.worker.clone();

        let (rem_rx, score_rx, mut error_rx, mut complete_rx) = {
            let cx = worker.context();
            let session_guard = cx.session();
            let session = session_guard.as_ref().expect("session should exist");

            (
                session.remaining_time(),
                session.score(),
                session.error(),
                session.complete(),
            )
        };

        let rem_cancel = {
            let rem_stream = WatchStream::new(rem_rx).fuse();
            let rem_prop = self.state.remaining_time.clone();
            rem_prop.bind(rem_stream)
        };

        let score_cancel = {
            let score_stream = WatchStream::new(score_rx)
                .map(|v| v.try_into().expect("score overflowed"))
                .fuse();
            let score_prop = self.state.score.clone();
            score_prop.bind(score_stream)
        };

        // Handle error_rx.
        slint::spawn_local({
            let nc = self.nav_controller.clone();
            async move {
                loop {
                    if error_rx.changed().await.is_err() {
                        // session ended and all error_tx has dropped
                        break;
                    }
                    if let Some(err) = error_rx.borrow_and_update().as_ref() {
                        nc.navigate(NavRoute::Fallback(err.to_string()));
                        // TODO: breakすべきか?
                    }
                }
            }
        })
        .unwrap();

        // Handle complete_rx.
        slint::spawn_local({
            let nc = self.nav_controller.clone();
            async move {
                complete_rx.recv().await.unwrap();
                rem_cancel.cancel();
                score_cancel.cancel();

                nc.navigate(NavRoute::Score { game_id });
            }
        })
        .unwrap()
    }
}

impl<C> PlayingViewModelTrait for PlayingViewModel<C> {}

pub struct PlayingDestination(
    #[allow(dead_code)] // may used when some arg is added to the route
    PlayingViewModelRc<Context>,
);

impl PlayingDestination {
    pub fn new(application: &Application) -> Self {
        Self(PlayingViewModelRc::new(application))
    }
}

impl NavDestination<NavRoute> for PlayingDestination {
    fn load(&self, route: &NavRoute) {
        debug!("loading PlayingViewModel");

        let NavRoute::Playing(_difficulty, game_id) = route else {
            panic!("matched variant should be given");
        };

        self.0.borrow().listen_session(*game_id);
    }

    fn route(&self) -> NavRouteKind {
        NavRouteKind::Playing
    }
}

pub mod test_utils {
    use std::{cell::RefCell, rc::Rc};

    use parking_lot::RwLock;
    use slint::SharedString;
    use tokio::sync::{broadcast, watch};

    use touchpanel_ui::{NavRoute, UiNavRoute};

    use crate::data::{DisplayableRemainingTime, RequestError};

    use super::*;

    /// Fake for [`SessionProvider`].
    pub struct FakeSessionProvider {
        session: RwLock<Option<Session>>,
    }

    impl FakeSessionProvider {
        pub fn drop_session(&self) {
            let mut guard = self.session.write();
            *guard = None;
        }
    }

    impl SessionProvider for FakeSessionProvider {
        fn session(&self) -> RwLockReadGuard<'_, Option<Session>> {
            self.session.read()
        }
    }

    pub struct PlayingScreenTest {
        pub route: Rc<RefCell<UiNavRoute>>,
        pub remaining_time: Rc<RefCell<SharedString>>,
        pub score: Rc<RefCell<i32>>,
        pub worker: WorkerThread<FakeSessionProvider>,
        pub vm: PlayingViewModel<FakeSessionProvider>,

        pub score_tx: watch::Sender<u32>,
        pub rem_tx: watch::Sender<DisplayableRemainingTime>,
        pub error_tx: watch::Sender<Option<RequestError>>,
        pub complete_tx: broadcast::Sender<()>,
    }

    impl PlayingScreenTest {
        pub fn new() -> Self {
            let route = Rc::new(RefCell::new(UiNavRoute::Start));

            let nav_controller = NavController::new(NavRoute::Start, {
                let route = Rc::clone(&route);
                move |new| {
                    let mut guard = route.borrow_mut();
                    *guard = new.into();
                }
            });
            let (score_tx, score_rx) = watch::channel(0);
            let (rem_tx, rem_rx) = watch::channel(DisplayableRemainingTime::ZERO);
            let (error_tx, error_rx) = watch::channel(None);
            let (complete_tx, complete_rx) = broadcast::channel(8);
            let worker = WorkerThread::new(FakeSessionProvider {
                session: RwLock::new(Some(Session::new(
                    struckout_proto::Difficulty::Normal,
                    score_tx.clone(),
                    rem_tx.clone(),
                    error_tx.clone(),
                    complete_tx.clone(),
                ))),
            });
            let (state, mock) = PlayingStates::new_mocked();
            let vm = PlayingViewModel {
                nav_controller,
                worker: worker.clone(),
                state,
            };

            Self {
                route,
                remaining_time: mock.remaining_time,
                score: mock.score,
                worker,
                vm,

                score_tx,
                rem_tx,
                error_tx,
                complete_tx,
            }
        }
    }
}
