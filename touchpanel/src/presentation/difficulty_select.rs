use slint::{ComponentHandle, Global};
use tokio::sync::oneshot;

use crate::{Application, Context, NavController};
use stern::{
    WorkerThread,
    nav::NavDestination,
    worker::{ForegroundExecutor, SlintExecutor},
};
use struckout_proto::types::PlayerId;
use touchpanel_ui::{
    DifficulitySelectPropertyMappers, DifficulitySelectStates, DifficulitySelectViewModelTrait,
    NavRoute, NavRouteKind,
};
use tracing::{debug, trace};

touchpanel_ui::define_difficulity_select_mapper! {}

viewmodel_rc!(
    DifficulitySelectViewModel<SlintExecutor>,
    DifficulitySelectAdopter
);

#[derive(Debug)]
struct DifficulitySelectViewModel<E> {
    nav_controller: NavController,
    worker: WorkerThread<Context, E>,
    state: DifficulitySelectStates<Mapper>,
    player_id: Option<PlayerId>,
}

impl DifficulitySelectViewModel<SlintExecutor> {
    fn new(application: &Application) -> Self {
        Self {
            nav_controller: application.nav_controller.clone(),
            worker: application.worker.clone(),
            state: DifficulitySelectStates::<Mapper>::new(
                application
                    .ui
                    .global::<touchpanel_ui::DifficulitySelectAdopter>()
                    .as_weak(),
            ),
            player_id: None,
        }
    }
}

impl<E> DifficulitySelectViewModelTrait for DifficulitySelectViewModel<E>
where
    E: ForegroundExecutor,
{
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
            let mut gm = cx.game_master.get().unwrap().clone();
            let res = gm.start_game(player_id, difficulty).await;
            tx.send(res).unwrap();
        });
        slint::spawn_local(async move {
            match rx.await.unwrap() {
                Ok(game_id) => {
                    nc.navigate(NavRoute::Playing(difficulty_ui, game_id));
                }
                Err(e) => {
                    nc.navigate(NavRoute::Fallback(e.to_string()));
                }
            }
        })
        .unwrap();
    }

    fn on_select_difficulity(&mut self, val: touchpanel_ui::Difficulity) {
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
    fn load(&mut self, route: &NavRoute) {
        debug!("loading DifficultySelectViewModel");
        let NavRoute::DifficulitySelect { player_id } = route else {
            panic!("matched variant should be given");
        };

        self.0.borrow_mut().player_id = Some(*player_id);
    }

    fn route(&self) -> NavRouteKind {
        NavRouteKind::DifficulitySelect
    }
}
