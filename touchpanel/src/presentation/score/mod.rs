use crate::{
    Application, Context, NavController,
    ui::{self, NavRoute, NavRouteKind, ScoreStates, ScoreViewModelTrait},
};
use slint::{ComponentHandle, Global};
use stern::{WorkerThread, nav::NavDestination};
use tracing::debug;

viewmodel_rc!(ScoreViewModel, ScoreAdopter);

#[derive(Debug)]
struct ScoreViewModel {
    nav_controller: NavController,
    state: ScoreStates,
}

impl ScoreViewModel {
    fn new(application: &Application) -> Self {
        Self {
            nav_controller: application.nav_controller.clone(),
            state: ScoreStates::new(application.ui.global::<ui::ScoreAdopter>().as_weak()),
        }
    }
}

impl ScoreViewModelTrait for ScoreViewModel {
    fn on_next_clicked(&mut self) {
        self.nav_controller.navigate(NavRoute::Ranking);
    }
}

pub struct ScoreDestination {
    worker: WorkerThread<Context>,
    viewmodel: ScoreViewModelRc,
}

impl ScoreDestination {
    pub fn new(application: &Application) -> Self {
        Self {
            worker: application.worker.clone(),
            viewmodel: ScoreViewModelRc::new(application),
        }
    }
}

impl NavDestination<NavRoute> for ScoreDestination {
    fn load(&self, route: &NavRoute) {
        debug!("loading ScoreViewModel");
        let NavRoute::Score = route else {
            panic!("matched variant should be given");
        };

        self.worker.spawn_cx(async move |cx| {
            let mut guard = cx.write();
            // context is initialized before navigated to StartScreen
            let gm = guard.game_master.get_mut().unwrap();
        });

        /*let session = self
            .session_manager
            .borrow()
            .session()
            .expect("session should exist when ScoreDestination is loaded");
        viewmodel.state.rank.set(ui::Rank::S); //TODO: 難易度に応じて切り替える
        viewmodel
            .state
            .score
            .set(session.cur_score().try_into().unwrap());*/

        todo!()
    }

    fn route(&self) -> NavRouteKind {
        NavRouteKind::Score
    }
}
