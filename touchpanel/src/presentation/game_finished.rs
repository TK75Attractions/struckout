use slint::{ComponentHandle, Global, ToSharedString};
use stern::{WorkerThread, nav::NavDestination, worker::SlintExecutor};
use tokio::sync::oneshot;
use touchpanel_ui::{
    GameFinishedPropertyMappers, GameFinishedStates, GameFinishedViewModelTrait, NavRoute,
    NavRouteKind,
};

use crate::{Application, Context, NavController};

touchpanel_ui::define_game_finished_mapper! {}
viewmodel_rc!(GameFinishedViewModel, GameFinishedAdopter);

struct GameFinishedViewModel {
    worker: WorkerThread<Context, SlintExecutor>,
    nav_controller: NavController,
    state: GameFinishedStates<Mapper>,
}

impl GameFinishedViewModel {
    fn new(application: &Application) -> Self {
        Self {
            worker: application.worker.clone(),
            nav_controller: application.nav_controller.clone(),
            state: GameFinishedStates::new(
                application
                    .ui
                    .global::<touchpanel_ui::GameFinishedAdopter>()
                    .as_weak(),
            ),
        }
    }
}

impl GameFinishedViewModelTrait for GameFinishedViewModel {
    fn on_next_clicked(&mut self) {
        if self.state.busy.get() {
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
            let _ = tx.send(gm.go_ranking_screen().await);
        });
        self.worker.spawn_local({
            let busy = self.state.busy.clone();
            let error_msg = self.state.error_msg.clone();
            let nc = self.nav_controller.clone();
            async move {
                let result = rx.await.expect("ranking request must complete");
                busy.set(false);
                match result {
                    Ok(()) => nc.navigate(NavRoute::RankingControl),
                    Err(error) => error_msg.set(
                        format!("ランキング画面への切り替えに失敗しました: {error}")
                            .to_shared_string(),
                    ),
                }
            }
        });
    }
}

pub struct GameFinishedDestination(GameFinishedViewModelRc);
impl GameFinishedDestination {
    pub fn new(application: &Application) -> Self {
        Self(GameFinishedViewModelRc::new(application))
    }
}
impl NavDestination<NavRoute> for GameFinishedDestination {
    fn load(&mut self, route: &NavRoute) {
        let NavRoute::GameFinished = route else {
            unreachable!("score route is required")
        };
        let vm = self.0.borrow();
        vm.state.busy.set(false);
        vm.state.error_msg.set("".into());
    }
    fn route(&self) -> NavRouteKind {
        NavRouteKind::GameFinished
    }
}
