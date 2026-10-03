use touchpanel_ui::{NameInputNonFirstStates, NameInputNonFirstViewModelTrait, NameInputNonFirstPropertyMappers, NavRoute, NavRouteKind};
use stern::nav::NavDestination;

use crate::Application;

touchpanel_ui::define_name_input_non_first_mapper! {}

struct NameInputNonFirstViewModel {
    state: NameInputNonFirstStates<Mapper>,
}

impl NameInputNonFirstViewModel {
    fn new(application: &Application) -> Self {
        use slint::{ComponentHandle, Global};
        Self {
            state: NameInputNonFirstStates::<Mapper>::new(
                application.ui.global::<touchpanel_ui::NameInputNonFirstAdopter>().as_weak(),
            )
        }
    }
}

impl NameInputNonFirstViewModelTrait for NameInputNonFirstViewModel {}

pub struct NameInputNonFirstDestination(NameInputNonFirstViewModel);

impl NameInputNonFirstDestination {
    pub fn new(application: &Application) -> Self {
        Self(NameInputNonFirstViewModel::new(application))
    }
}

impl NavDestination<NavRoute> for NameInputNonFirstDestination {
    fn load(&mut self, route: &NavRoute) {
        let NavRoute::NameInputNonFirst = route else {
            panic!("given NavRoute has invalid variant value");
        };

        todo!()
    }

    fn route(&self) -> NavRouteKind {
        NavRouteKind::NameInputNonFirst
    }
}
