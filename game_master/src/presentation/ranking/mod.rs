use tracing::debug;

use crate::{
    Application,
    nav::{NavDestination, NavRouteKind},
};

pub struct RankingDestination {}

impl RankingDestination {
    pub fn new<PT>(_application: &Application<PT>) -> Self {
        Self {}
    }
}

impl NavDestination for RankingDestination {
    fn load(&self, route: &crate::nav::NavRoute) {
        debug!("loading RankingScreen")
    }

    fn route(&self) -> NavRouteKind {
        NavRouteKind::Ranking
    }
}
