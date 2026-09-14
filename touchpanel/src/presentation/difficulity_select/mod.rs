use slint::{ComponentHandle, Global};
use tokio::sync::oneshot;

use crate::{
    Application, Context, NavController,
    data::PlayerId,
    ui::{self, DifficulitySelectStates, DifficulitySelectViewModelTrait, NavRoute, NavRouteKind},
};
use stern::{GlobalExt as _, WorkerThread, nav::NavDestination};
use tracing::{debug, trace};

viewmodel_rc!(DifficulitySelectViewModel, DifficulitySelectAdopter);

#[derive(Debug)]
struct DifficulitySelectViewModel {
    nav_controller: NavController,
    worker: WorkerThread<Context>,
    state: DifficulitySelectStates,
    player_id: Option<PlayerId>,
}

impl DifficulitySelectViewModel {
    fn new(application: &Application) -> Self {
        Self {
            nav_controller: application.nav_controller.clone(),
            worker: application.worker.clone(),
            state: DifficulitySelectStates::new(
                application
                    .ui
                    .global::<ui::DifficulitySelectAdopter>()
                    .as_weak(),
            ),
            player_id: None,
        }
    }
}

impl DifficulitySelectViewModelTrait for DifficulitySelectViewModel {
    fn on_start_game(&mut self) {
        trace!("DifficulitySelectViewModel::on_start_game");

        let nc = self.nav_controller.clone();
        let (tx, rx) = oneshot::channel();

        let difficulty_ui = self.state.selected_difficulity.get();
        let difficulty = difficulty_ui.into();
        let player_id = self
            .player_id
            .expect("should be some while this screen is shown");

        self.worker.spawn_cx(async move |cx| {
            let mut gm = {
                let guard = cx.read();
                guard.game_master.get().unwrap().clone()
            };
            let res = gm.start_game(player_id, difficulty).await;
            tx.send(res).unwrap();
        });
        slint::spawn_local(async move {
            match rx.await.unwrap() {
                Ok(_) => {
                    nc.navigate(NavRoute::Playing(difficulty_ui));
                }
                Err(e) => {
                    nc.navigate(NavRoute::Fallback(e.to_string()));
                }
            }
        })
        .unwrap();
    }

    fn on_select_difficulity(&mut self, val: ui::Difficulity) {
        trace!(?val, "DifficulitySelectViewModel::on_select_difficulity");
        self.state.selected_difficulity.set(val);
    }
}

pub struct DifficultySelectDestination(
    #[allow(unused)] // may used when some arg is added to the route
    DifficulitySelectViewModelRc,
);

impl DifficultySelectDestination {
    pub fn new(application: &Application) -> Self {
        Self(DifficulitySelectViewModelRc::new(application))
    }
}

impl NavDestination<NavRoute> for DifficultySelectDestination {
    fn load(&self, route: &NavRoute) {
        debug!("loading DifficultySelectViewModel");
        let NavRoute::DifficulitySelect { player_id } = route else {
            panic!("matched variant should be given");
        };

        let mut vm = self.0.borrow_mut();
        vm.player_id = Some(*player_id);
    }

    fn route(&self) -> NavRouteKind {
        NavRouteKind::DifficulitySelect
    }
}
