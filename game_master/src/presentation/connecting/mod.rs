use crate::{
    Application,
    ui::{NavRoute, NavRouteKind},
};
use slint_fw::nav::NavDestination;
use tracing::debug;

pub struct ConnectingDestination(());

impl ConnectingDestination {
    pub fn new(_application: &Application) -> Self {
        Self(())
    }
}

impl NavDestination<NavRoute> for ConnectingDestination {
    fn load(&self, route: &NavRoute) {
        debug!("loading ConnectingScreen");

        let NavRoute::Connecting = route else {
            panic!("matched variant should be given");
        };
    }

    fn route(&self) -> NavRouteKind {
        NavRouteKind::Connecting
    }
}
