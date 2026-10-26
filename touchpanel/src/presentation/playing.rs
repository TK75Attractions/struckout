use futures_util::StreamExt;
use slint::{ComponentHandle, Global, ToSharedString};
use stern::{
    WorkerThread,
    nav::NavDestination,
    worker::{ForegroundExecutor, SlintExecutor},
};
use tokio::sync::watch;
use tokio_stream::wrappers::WatchStream;
use tokio_util::sync::CancellationToken;
use touchpanel_ui::{
    NavRoute, NavRouteKind, PlayingPropertyMappers, PlayingStates, PlayingViewModelTrait,
};

use crate::{
    Application, Context,
    data::{DisplayableRemainingTime, Session},
};

viewmodel_rc!(PlayingViewModel<Context, SlintExecutor>, PlayingAdopter);

touchpanel_ui::define_playing_mapper! {
    remaining_time to DisplayableRemainingTime { |value: DisplayableRemainingTime| value.to_shared_string() },
    score to u32 { |value: u32| value.to_shared_string() },
}

struct PlayingViewModel<C, E> {
    worker: WorkerThread<C, E>,
    state: PlayingStates<Mapper>,
    bindings: Vec<CancellationToken>,
}

trait SessionProvider: Send + Sync + 'static {
    fn session(&self) -> watch::Receiver<Session>;
}
impl SessionProvider for Context {
    fn session(&self) -> watch::Receiver<Session> {
        self.game_master
            .get()
            .expect("connection is initialized")
            .session()
    }
}

impl PlayingViewModel<Context, SlintExecutor> {
    fn new(application: &Application) -> Self {
        Self {
            worker: application.worker.clone(),
            state: PlayingStates::new(
                application
                    .ui
                    .global::<touchpanel_ui::PlayingAdopter>()
                    .as_weak(),
            ),
            bindings: Vec::new(),
        }
    }
}

impl<C: SessionProvider, E: ForegroundExecutor> PlayingViewModel<C, E> {
    fn listen_session(&mut self) {
        for token in self.bindings.drain(..) {
            token.cancel();
        }
        let session = self.worker.background_executor().context().session();
        self.bindings.push(
            self.state.score.clone().bind(
                &self.worker,
                WatchStream::new(session.clone())
                    .fuse()
                    .map(|session| session.score),
            ),
        );
        self.bindings.push(
            self.state.remaining_time.clone().bind(
                &self.worker,
                WatchStream::new(session)
                    .fuse()
                    .map(|session| session.remaining_time),
            ),
        );
    }
}

impl<C, E> PlayingViewModelTrait for PlayingViewModel<C, E> {}

pub struct PlayingDestination(PlayingViewModelRc);
impl PlayingDestination {
    pub fn new(application: &Application) -> Self {
        Self(PlayingViewModelRc::new(application))
    }
}
impl NavDestination<NavRoute> for PlayingDestination {
    fn load(&mut self, route: &NavRoute) {
        let NavRoute::Playing = route else {
            unreachable!("playing route is required")
        };
        self.0.borrow_mut().listen_session();
    }
    fn route(&self) -> NavRouteKind {
        NavRouteKind::Playing
    }
}
