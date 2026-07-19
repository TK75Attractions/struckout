use std::rc::Rc;

use slint::{ComponentHandle, SharedString, ToSharedString};

use crate::{
    Application,
    ui::{self, NavRoute, NavRouteKind},
};
use slint_fw::nav::NavDestination;

state_struct!(Fallback, msg => SharedString);

#[derive(Debug)]
pub struct FallbackViewModel {
    state: FallbackState,
}

impl FallbackViewModel {
    fn new(adopter: &ui::FallbackAdopter) -> Self {
        Self {
            state: FallbackState::new(&adopter),
        }
    }
}

pub struct FallbackDestination {
    adopter: slint::Weak<ui::FallbackAdopter<'static>>,
}

impl FallbackDestination {
    pub fn new(application: &Application) -> Self {
        Self {
            adopter: application.ui.global::<ui::FallbackAdopter>().as_weak(),
        }
    }
}

impl NavDestination<NavRoute> for FallbackDestination {
    fn load(&self, route: &NavRoute) {
        let NavRoute::Fallback(msg) = route else {
            panic!("matched variant should be given");
        };

        let adopter = self.adopter.unwrap();
        let viewmodel = Rc::new(FallbackViewModel::new(&adopter));
        viewmodel.state.msg.set(msg.to_shared_string());
    }

    fn route(&self) -> NavRouteKind {
        NavRouteKind::Fallback
    }
}
