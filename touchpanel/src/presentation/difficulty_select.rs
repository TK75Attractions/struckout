use futures_util::StreamExt;
use slint::{ComponentHandle, Global, ToSharedString};
use stern::{WorkerThread, nav::NavDestination, worker::SlintExecutor};
use struckout_proto::Difficulty;
use tokio::sync::oneshot;
use tokio_stream::wrappers::WatchStream;
use tokio_util::sync::CancellationToken;
use touchpanel_ui::{
    DifficulitySelectPropertyMappers, DifficulitySelectStates, DifficulitySelectViewModelTrait,
    NavRoute, NavRouteKind,
};

use crate::{Application, Context, data::SessionPhase};

touchpanel_ui::define_difficulity_select_mapper! {}
viewmodel_rc!(DifficulitySelectViewModel, DifficulitySelectAdopter);

struct DifficulitySelectViewModel {
    worker: WorkerThread<Context, SlintExecutor>,
    state: DifficulitySelectStates<Mapper>,
    bindings: Vec<CancellationToken>,
}

impl DifficulitySelectViewModel {
    fn new(application: &Application) -> Self {
        Self {
            worker: application.worker.clone(),
            state: DifficulitySelectStates::new(
                application
                    .ui
                    .global::<touchpanel_ui::DifficulitySelectAdopter>()
                    .as_weak(),
            ),
            bindings: Vec::new(),
        }
    }

    fn load(&mut self) {
        for token in self.bindings.drain(..) {
            token.cancel();
        }
        self.state.busy.set(false);
        self.state.error_msg.set("".into());
        let session = self
            .worker
            .context()
            .game_master
            .get()
            .expect("connection is initialized")
            .session();
        self.bindings.push(
            self.state.selected_difficulity.clone().bind(
                &self.worker,
                WatchStream::new(session.clone())
                    .fuse()
                    .map(|session| session.difficulty.unwrap_or(Difficulty::Normal).into()),
            ),
        );
        self.bindings.push(
            self.state.ready.clone().bind(
                &self.worker,
                WatchStream::new(session.clone())
                    .fuse()
                    .map(|session| session.phase == SessionPhase::Ready),
            ),
        );
        if session.borrow().difficulty.is_none() {
            self.set_difficulty(Difficulty::Normal);
        }
    }

    fn set_difficulty(&self, difficulty: Difficulty) {
        if self.state.busy.get() || self.state.ready.get() {
            return;
        }
        self.state.busy.set(true);
        self.state.error_msg.set("".into());
        let (tx, rx) = oneshot::channel();
        self.worker.spawn_cx(async move |cx| {
            let mut gm = cx
                .game_master
                .get()
                .expect("connection is initialized")
                .clone();
            let _ = tx.send(gm.set_session_difficulty(difficulty).await);
        });
        self.worker.spawn_local({
            let busy = self.state.busy.clone();
            let error_msg = self.state.error_msg.clone();
            async move {
                let result = rx.await.expect("difficulty request must complete");
                busy.set(false);
                if let Err(error) = result {
                    error_msg
                        .set(format!("難易度の設定に失敗しました: {error}").to_shared_string());
                }
            }
        });
    }
}

impl DifficulitySelectViewModelTrait for DifficulitySelectViewModel {
    fn on_start_game(&mut self) {
        if self.state.busy.get() || self.state.ready.get() {
            return;
        }
        self.state.busy.set(true);
        self.state.error_msg.set("".into());
        let (tx, rx) = oneshot::channel();
        self.worker.spawn_cx(async move |cx| {
            let mut gm = cx
                .game_master
                .get()
                .expect("connection is initialized")
                .clone();
            let _ = tx.send(gm.set_session_ready().await);
        });
        self.worker.spawn_local({
            let busy = self.state.busy.clone();
            let error_msg = self.state.error_msg.clone();
            async move {
                let result = rx.await.expect("readiness request must complete");
                busy.set(false);
                if let Err(error) = result {
                    error_msg
                        .set(format!("準備完了の送信に失敗しました: {error}").to_shared_string());
                }
            }
        });
    }

    fn on_select_difficulity(&mut self, value: touchpanel_ui::Difficulity) {
        self.set_difficulty(value.into());
    }
}

pub struct DifficultySelectDestination(DifficulitySelectViewModelRc);
impl DifficultySelectDestination {
    pub fn new(application: &Application) -> Self {
        Self(DifficulitySelectViewModelRc::new(application))
    }
}
impl NavDestination<NavRoute> for DifficultySelectDestination {
    fn load(&mut self, route: &NavRoute) {
        let NavRoute::DifficulitySelect = route else {
            unreachable!("difficulty selection route is required")
        };
        self.0.borrow_mut().load();
    }
    fn route(&self) -> NavRouteKind {
        NavRouteKind::DifficulitySelect
    }
}
