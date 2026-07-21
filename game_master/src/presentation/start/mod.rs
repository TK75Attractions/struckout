use slint::{ComponentHandle, Global};
use slint_fw::nav::NavDestination;
use tracing::{debug, trace};

use crate::{
    Application, NavController,
    ui::{self, NavRoute, NavRouteKind, StartStates, StartViewModelTrait},
};

viewmodel_rc!(StartViewModel, StartAdopter);

#[derive(Debug)]
struct StartViewModel {
    nav_controller: NavController,
    _state: StartStates,
}

impl StartViewModel {
    fn new(application: &Application) -> Self {
        Self {
            nav_controller: application.nav_controller.clone(),
            _state: StartStates::new(application.ui.global::<ui::StartAdopter>().as_weak()),
        }
    }
}

impl StartViewModelTrait for StartViewModel {
    fn on_click(&mut self) -> () {
        trace!("StartScreen::on_click()");
        self.nav_controller.navigate(NavRoute::NameInput);
    }
}

pub struct StartScreenDestination(
    #[allow(unused_variables)] // just for viewmodel initialization
    StartViewModelRc,
);

impl StartScreenDestination {
    pub fn new(application: &Application) -> Self {
        Self(StartViewModelRc::new(application))
    }
}

impl NavDestination<NavRoute> for StartScreenDestination {
    fn load(&self, route: &NavRoute) {
        debug!("loading StartScreenViewModel");

        let NavRoute::Start = route else {
            panic!("matched variant should be given");
        };
    }

    fn route(&self) -> NavRouteKind {
        NavRouteKind::Start
    }
}
