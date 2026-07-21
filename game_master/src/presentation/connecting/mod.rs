use crate::{
    Application,
    ui::{self, ConnectingStates, ConnectingViewModelTrait, NavRoute, NavRouteKind},
};
use slint::{ComponentHandle as _, Global as _};
use slint_fw::nav::NavDestination;
use tracing::debug;

viewmodel_rc!(ConnectingViewModel, ConnectingAdopter);

struct ConnectingViewModel {
    _state: ConnectingStates,
}

impl ConnectingViewModel {
    fn new(application: &Application) -> Self {
        Self {
            _state: ConnectingStates::new(
                application.ui.global::<ui::ConnectingAdopter>().as_weak(),
            ),
        }
    }
}

impl ConnectingViewModelTrait for ConnectingViewModel {}

pub struct ConnectingDestination(
    #[allow(unused)] // just for initialize viewmodel
    ConnectingViewModelRc,
);

impl ConnectingDestination {
    pub fn new(application: &Application) -> Self {
        Self(ConnectingViewModelRc::new(application))
    }
}

impl NavDestination<NavRoute> for ConnectingDestination {
    fn load(&self, route: &NavRoute) {
        debug!("loading ConnectingScreen");

        let NavRoute::Connecting = route else {
            panic!("matched variant should be given");
        };
        // do nothing because the route doesn't have any arguments
    }

    fn route(&self) -> NavRouteKind {
        NavRouteKind::Connecting
    }
}
