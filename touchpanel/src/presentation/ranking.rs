use tracing::debug;

use crate::Application;
use slint::{ComponentHandle as _, Global as _};
use stern::nav::NavDestination;
use touchpanel_ui::{
    NavRoute, NavRouteKind, RankingPropertyMappers, RankingStates, RankingViewModelTrait,
};

touchpanel_ui::define_ranking_mapper! {}

viewmodel_rc!(RankingViewModel, RankingAdopter);

struct RankingViewModel {
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
        }
    }
}

impl RankingViewModelTrait for RankingViewModel {}

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
    fn load(&self, route: &touchpanel_ui::NavRoute) {
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
