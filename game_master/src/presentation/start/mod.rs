use std::rc::Rc;

use slint::{ComponentHandle, Global};
use tracing::{debug, trace};

use crate::{
    Application,
    nav::{NavController, NavDestination, NavRoute, NavRouteKind},
    ui,
};

#[derive(Debug)]
pub struct StartScreenViewModel {
    nav_controller: NavController,
}

impl StartScreenViewModel {
    pub fn new(nav_controller: NavController) -> Self {
        Self { nav_controller }
    }

    pub fn on_click(&self) {
        trace!("StartScreen::on_click()");
        self.nav_controller.navigate(NavRoute::NameInput);
    }
}

pub struct StartScreenDestination {
    adopter: slint::Weak<ui::StartScreenAdopter<'static>>,
    nav_controller: NavController,
}

impl StartScreenDestination {
    pub fn new(application: &Application) -> Self {
        Self {
            adopter: application.ui.global::<ui::StartScreenAdopter>().as_weak(),
            nav_controller: application.nav_controller.clone(),
        }
    }
}

impl NavDestination for StartScreenDestination {
    fn load(&self, route: &NavRoute) {
        debug!("loading StartScreenViewModel");

        let NavRoute::Start = route else {
            panic!("matched variant should be given");
        };

        let adopter = self.adopter.unwrap();
        let viewmodel = Rc::new(StartScreenViewModel::new(self.nav_controller.clone()));

        adopter.on_click({
            let viewmodel = Rc::clone(&viewmodel);
            move || {
                viewmodel.on_click();
            }
        });
    }

    fn route(&self) -> NavRouteKind {
        NavRouteKind::Start
    }
}
