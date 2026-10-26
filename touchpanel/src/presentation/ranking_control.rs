use slint::{ComponentHandle, Global, ToSharedString};
use stern::{WorkerThread, nav::NavDestination, worker::SlintExecutor};
use tokio::sync::oneshot;
use touchpanel_ui::{
    NavRoute, NavRouteKind, RankingControlPropertyMappers, RankingControlStates,
    RankingControlViewModelTrait,
};

use crate::{Application, Context, NavController};

touchpanel_ui::define_ranking_control_mapper! {}
viewmodel_rc!(RankingControlViewModel, RankingControlAdopter);

struct RankingControlViewModel {
    worker: WorkerThread<Context, SlintExecutor>,
    nav_controller: NavController,
    state: RankingControlStates<Mapper>,
}

impl RankingControlViewModel {
    fn new(application: &Application) -> Self {
        Self {
            worker: application.worker.clone(),
            nav_controller: application.nav_controller.clone(),
            state: RankingControlStates::new(
                application
                    .ui
                    .global::<touchpanel_ui::RankingControlAdopter>()
                    .as_weak(),
            ),
        }
    }
}

impl RankingControlViewModelTrait for RankingControlViewModel {
    fn on_move_to_start_screen(&mut self) {
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
            let _ = tx.send(gm.go_home_screen().await);
        });
        self.worker.spawn_local({
            let busy = self.state.busy.clone();
            let error_msg = self.state.error_msg.clone();
            let nc = self.nav_controller.clone();
            async move {
                let result = rx.await.expect("home request must complete");
                busy.set(false);
                match result {
                    Ok(()) => nc.navigate(NavRoute::Start),
                    Err(error) => error_msg.set(
                        format!("ホーム画面への切り替えに失敗しました: {error}").to_shared_string(),
                    ),
                }
            }
        });
    }
}

pub struct RankingControlDestination(RankingControlViewModelRc);
impl RankingControlDestination {
    pub fn new(application: &Application) -> Self {
        Self(RankingControlViewModelRc::new(application))
    }
}
impl NavDestination<NavRoute> for RankingControlDestination {
    fn load(&mut self, route: &NavRoute) {
        let NavRoute::RankingControl = route else {
            unreachable!("ranking route is required")
        };
        let vm = self.0.borrow();
        vm.state.busy.set(false);
        vm.state.error_msg.set("".into());
    }
    fn route(&self) -> NavRouteKind {
        NavRouteKind::RankingControl
    }
}
