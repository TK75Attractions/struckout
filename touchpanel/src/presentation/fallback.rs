use slint::{ComponentHandle, Global, ToSharedString};

use crate::Application;
use stern::{GlobalExt, nav::NavDestination};
use touchpanel_ui::{
    FallbackPropertyMappers, FallbackStates, FallbackViewModelTrait, NavRoute, NavRouteKind,
};

touchpanel_ui::define_fallback_mapper! {}

viewmodel_rc!(FallbackViewModel, FallbackAdopter);

#[derive(Debug)]
struct FallbackViewModel {
    state: FallbackStates<Mapper>,
}

impl FallbackViewModel {
    fn new(application: &Application) -> Self {
        Self {
            state: FallbackStates::<Mapper>::new(
                application
                    .ui
                    .global::<touchpanel_ui::FallbackAdopter>()
                    .as_weak(),
            ),
        }
    }
}

impl FallbackViewModelTrait for FallbackViewModel {}

pub struct FallbackDestination(FallbackViewModelRc);

impl FallbackDestination {
    pub fn new(application: &Application) -> Self {
        Self(FallbackViewModelRc::new(application))
    }
}

impl NavDestination<NavRoute> for FallbackDestination {
    fn load(&self, route: &NavRoute) {
        let NavRoute::Fallback(msg) = route else {
            panic!("matched variant should be given");
        };

        self.0.borrow().state.msg.set(msg.to_shared_string());
    }

    fn route(&self) -> NavRouteKind {
        NavRouteKind::Fallback
    }
}
