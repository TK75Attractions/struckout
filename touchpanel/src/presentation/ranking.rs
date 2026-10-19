use tracing::{debug, instrument};

use crate::{Application, NavController};
use slint::{ComponentHandle as _, Global as _};
use stern::nav::NavDestination;
use touchpanel_ui::{
    NavRoute, NavRouteKind, RankingPropertyMappers, RankingStates, RankingViewModelTrait,
};

touchpanel_ui::define_ranking_mapper! {}

viewmodel_rc!(RankingViewModel, RankingAdopter);

#[derive(Debug)]
struct RankingViewModel {
    nav_controller: NavController,
    _state: RankingStates<Mapper>,
}

impl RankingViewModel {
    fn new(application: &Application) -> Self {
        Self {
            _state: RankingStates::<Mapper>::new(
                application
                    .ui
                    .global::<touchpanel_ui::RankingAdopter>()
                    .as_weak(),
            ),
            nav_controller: application.nav_controller.clone(),
        }
    }
}

impl RankingViewModelTrait for RankingViewModel {
    #[instrument]
    fn on_move_to_start_screen(&mut self) {
        self.nav_controller.navigate(NavRoute::Start);
    }
}

pub struct RankingDestination(
    #[allow(unused)] // just for viewmodel initialization
    RankingViewModelRc,
);

impl RankingDestination {
    pub fn new(application: &Application) -> Self {
        Self(RankingViewModelRc::new(application))
    }
}

impl NavDestination<NavRoute> for RankingDestination {
    fn load(&mut self, route: &touchpanel_ui::NavRoute) {
        debug!("loading RankingScreen");

        let NavRoute::Ranking = route else {
            panic!("matched variant should be given");
        };

        // do nothing because the route doesn't have any arguments
    }

    fn route(&self) -> NavRouteKind {
        NavRouteKind::Ranking
    }
}
