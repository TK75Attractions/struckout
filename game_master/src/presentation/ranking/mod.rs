use tracing::debug;

use crate::{
    Application,
    ui::{NavRoute, NavRouteKind},
};
use slint_fw::nav::NavDestination;

pub struct RankingDestination {}

impl RankingDestination {
    pub fn new(_application: &Application) -> Self {
        Self {}
    }
}

impl NavDestination<NavRoute> for RankingDestination {
    fn load(&self, _route: &crate::ui::NavRoute) {
        debug!("loading RankingScreen")
    }

    fn route(&self) -> NavRouteKind {
        NavRouteKind::Ranking
    }
}
