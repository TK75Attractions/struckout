use std::rc::Rc;

use slint::{ComponentHandle, SharedString, ToSharedString};

use crate::{
    Application,
    nav::{NavDestination, NavRoute},
    ui,
};

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

    pub fn load(&self, msg: impl Into<SharedString>) {
        self.state.msg.set(msg.into());
    }
}

pub struct FallbackDestination {
    adopter: slint::Weak<ui::FallbackAdopter<'static>>,
}

impl FallbackDestination {
    pub fn new<PT>(application: &Application<PT>) -> Self {
        Self {
            adopter: application.ui.global::<ui::FallbackAdopter>().as_weak(),
        }
    }
}

impl NavDestination for FallbackDestination {
    fn load(&self, route: &NavRoute) {
        let NavRoute::Fallback(msg) = route else {
            panic!("matched variant should be given");
        };

        let adopter = self.adopter.unwrap();
        let viewmodel = Rc::new(FallbackViewModel::new(&adopter));
        viewmodel.state.msg.set(msg.to_shared_string());
    }

    fn matches(&self, route: &NavRoute) -> bool {
        matches!(route, &NavRoute::Fallback(_))
    }
}
