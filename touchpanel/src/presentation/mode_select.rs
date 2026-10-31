use slint::{ComponentHandle, Global, ToSharedString};
use stern::{WorkerThread, nav::NavDestination, worker::SlintExecutor};
use struckout_proto::{GameMode, types::PlayerId};
use tokio::sync::oneshot;
use touchpanel_ui::{
    ModeSelectPropertyMappers, ModeSelectStates, ModeSelectViewModelTrait, NavRoute, NavRouteKind,
};

use crate::{Application, Context, NavController};

touchpanel_ui::define_mode_select_mapper! {}
viewmodel_rc!(ModeSelectViewModel, ModeSelectAdopter);

struct ModeSelectViewModel {
    worker: WorkerThread<Context, SlintExecutor>,
    nav_controller: NavController,
    state: ModeSelectStates<Mapper>,
    player_id: Option<PlayerId>,
}

impl ModeSelectViewModel {
    fn new(application: &Application) -> Self {
        let state = ModeSelectStates::new(
            application
                .ui
                .global::<touchpanel_ui::ModeSelectAdopter>()
                .as_weak(),
        );
        state
            .multiplay_availability
            .set(application.config.multiplay_availability);

        Self {
            worker: application.worker.clone(),
            nav_controller: application.nav_controller.clone(),
            state,
            player_id: None,
        }
    }

    fn select_mode(&self, mode: GameMode) {
        if self.state.busy.get() {
            return;
        }
        self.state.busy.set(true);
        // A failed response cannot tell us whether StartSession was committed.
        self.state.back_enabled.set(false);
        self.state.error_msg.set("".into());
        let player_id = self.player_id.expect("mode selection requires a player");
        let (tx, rx) = oneshot::channel();
        self.worker.spawn_cx(async move |cx| {
            let mut gm = cx
                .game_master
                .get()
                .expect("connection is initialized")
                .clone();
            let result = async {
                gm.start_session(player_id).await?;
                gm.set_game_mode(mode).await
            }
            .await;
            let _ = tx.send(result);
        });
        self.worker.spawn_local({
            let busy = self.state.busy.clone();
            let error_msg = self.state.error_msg.clone();
            let nc = self.nav_controller.clone();
            async move {
                let result = rx.await.expect("mode request must complete");
                busy.set(false);
                match result {
                    Ok(()) => nc.navigate(NavRoute::DifficulitySelect),
                    Err(error) => error_msg
                        .set(format!("モードの設定に失敗しました: {error}").to_shared_string()),
                }
            }
        });
    }
}

impl ModeSelectViewModelTrait for ModeSelectViewModel {
    fn on_select_single(&mut self) {
        self.select_mode(GameMode::Single);
    }
    fn on_select_multi(&mut self) {
        self.select_mode(GameMode::Multi);
    }
}

pub struct ModeSelectDestination(ModeSelectViewModelRc);

impl ModeSelectDestination {
    pub fn new(application: &Application) -> Self {
        Self(ModeSelectViewModelRc::new(application))
    }
}

impl NavDestination<NavRoute> for ModeSelectDestination {
    fn load(&mut self, route: &NavRoute) {
        let NavRoute::ModeSelect { player_id } = route else {
            unreachable!("mode selection route is required")
        };
        let mut vm = self.0.borrow_mut();
        vm.player_id = Some(*player_id);
        vm.state.busy.set(false);
        vm.state.back_enabled.set(true);
        vm.state.error_msg.set("".into());
    }
    fn route(&self) -> NavRouteKind {
        NavRouteKind::ModeSelect
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use stern::{WorkerThread, nav::NavController};

    use super::*;

    #[test]
    fn viewmodel_passes_multiplay_availability_to_slint() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = touchpanel_ui::AppWindow::new().unwrap();
        let nav_controller = NavController::new(NavRoute::Connecting, {
            let ui = ui.as_weak();
            move |route| ui.unwrap().set_nav_route(route)
        });
        let application = Application {
            nav_controller,
            ui,
            worker: WorkerThread::new(Context::new_empty()),
            config: Arc::new(crate::Config {
                server_addr: "127.0.0.1".to_string(),
                machine_id: 1,
                multiplay_availability: touchpanel_ui::MultiPlayAvailability::Disabled,
                simple_mode: false,
                monitor: None,
            }),
        };

        let viewmodel = ModeSelectViewModel::new(&application);

        assert_eq!(
            viewmodel.state.multiplay_availability.get(),
            touchpanel_ui::MultiPlayAvailability::Disabled
        );
    }
}
