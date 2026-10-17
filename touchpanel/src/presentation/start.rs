use slint::{ComponentHandle, Global};
use stern::{PropertyHandle, nav::NavDestination};
use tracing::{debug, trace};

use crate::{Application, NavController};

use touchpanel_ui::{
    NavRoute, NavRouteKind, StartPropertyMappers, StartStates, StartViewModelTrait,
};

viewmodel_rc!(StartViewModel, StartAdopter);

touchpanel_ui::define_start_mapper! {}

#[derive(Debug)]
struct StartViewModel {
    nav_controller: NavController,
    _state: StartStates<Mapper>,
    show_topbar_prop: PropertyHandle<bool>,
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
            show_topbar_prop: application.show_topbar_prop(),
        }
    }
}

impl StartViewModelTrait for StartViewModel {
    fn on_click_first_play(&mut self) {
        trace!("StartScreen::on_click()");
        self.nav_controller.navigate(NavRoute::NameInputFirst);
    }

    fn on_click_non_first_play(&mut self) {
        trace!("StartScreen::on_click_non_first_play()");
        self.nav_controller.navigate(NavRoute::NameInputNonFirst);
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
    fn load(&mut self, route: &NavRoute) {
        debug!("loading StartScreenViewModel");

        let NavRoute::Start = route else {
            panic!("matched variant should be given");
        };

        let vm = self.0.borrow();
        vm.show_topbar_prop.set(false);
    }

    fn route(&self) -> NavRouteKind {
        NavRouteKind::Start
    }
}
