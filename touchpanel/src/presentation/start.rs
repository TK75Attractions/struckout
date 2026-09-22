use slint::{ComponentHandle, Global};
use stern::nav::NavDestination;
use tracing::{debug, trace};

use crate::{Application, NavController};

use touchpanel_ui::{
    NameInputMode, NavRoute, NavRouteKind, StartPropertyMappers, StartStates, StartViewModelTrait,
};

viewmodel_rc!(StartViewModel, StartAdopter);

touchpanel_ui::define_start_mapper! {}

#[derive(Debug)]
struct StartViewModel {
    nav_controller: NavController,
    _state: StartStates<Mapper>,
}

impl StartViewModel {
    fn new(application: &Application) -> Self {
        Self {
            nav_controller: application.nav_controller.clone(),
            _state: StartStates::<Mapper>::new(
                application
                    .ui
                    .global::<touchpanel_ui::StartAdopter>()
                    .as_weak(),
            ),
        }
    }
}

impl StartViewModelTrait for StartViewModel {
    fn on_click_first_play(&mut self) {
        trace!("StartScreen::on_click()");
        self.nav_controller
            .navigate(NavRoute::NameInput(NameInputMode::FirstPlay));
    }

    fn on_click_non_first_play(&mut self) {
        trace!("StartScreen::on_click_non_first_play()");
        self.nav_controller
            .navigate(NavRoute::NameInput(NameInputMode::NonFirstPlay));
    }
}

pub struct StartScreenDestination(
    #[allow(dead_code)] // just for viewmodel initialization
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
