use crate::{
    Application,
    nav::{NavDestination, NavRoute},
};
use tracing::debug;

pub struct ConnectingDestination(());

impl ConnectingDestination {
    pub fn new<PT>(_application: &Application<PT>) -> Self {
        Self(())
    }
}

impl NavDestination for ConnectingDestination {
    fn load(&self, route: &NavRoute) {
        debug!("loading ConnectingScreen");

        let NavRoute::Connecting = route else {
            panic!("matched variant should be given");
        };
    }

    fn matches(&self, route: &NavRoute) -> bool {
        matches!(route, &NavRoute::Connecting)
    }
}
