use std::cell::RefCell;
use std::rc::Rc;

use slint::{ComponentHandle, Global, SharedString, ToSharedString};

use crate::{
    Application, NavController,
    data::projector::{ProjectorTransport, ProjectorTransportTrait as _, StartGameError},
    ui::{self, NavRoute, NavRouteKind},
};
use slint_fw::nav::NavDestination;
use tracing::{debug, error, trace};

state_struct!(
    DifficulitySelect,
    selected_difficulity => ui::Difficulity,
    error_msg => SharedString
);

#[derive(Debug)]
pub struct DifficulitySelectViewModel {
    nav_controller: NavController,
    state: DifficulitySelectState,
    projector_transport: Rc<RefCell<ProjectorTransport>>,
}

impl DifficulitySelectViewModel {
    fn new(
        nav_controller: NavController,
        state: DifficulitySelectState,
        projector_transport: Rc<RefCell<ProjectorTransport>>,
    ) -> Self {
        Self {
            nav_controller,
            state,
            projector_transport,
        }
    }

    fn on_select_difficulity(&self, val: ui::Difficulity) {
        trace!(?val, "DifficulitySelectViewModel::on_select_difficulity");
        self.state.selected_difficulity.set(val);
    }

    fn on_start_game(&self) {
        trace!("DifficulitySelectViewModel::on_start_game");
        let difficulty = self.state.selected_difficulity.get();
        // let error_msg = self.state.error_msg.clone();
        let nav_controller = self.nav_controller.clone();
        self.projector_transport // TODO: SessionManagerに移すかも
            .borrow_mut()
            .start_game(difficulty.clone(), move |res| match res {
                Ok(()) => {
                    nav_controller.navigate(NavRoute::Playing(difficulty));
                }
                Err(StartGameError::NotConnected) => panic!("should be already connected"),
                Err(StartGameError::Tcp(e)) => {
                    error!(?e, "failed to send StartGame message to projector");
                    // error_msg.set("ネットワーク接続に失敗しました".to_shared_string());
                }
            });
    }
}

pub struct DifficultySelectDestination {
    adopter: slint::Weak<ui::DifficulitySelectAdopter<'static>>,
    nav_controller: NavController,
    projector_transport: Rc<RefCell<ProjectorTransport>>,
}

impl DifficultySelectDestination {
    pub fn new(application: &Application) -> Self {
        Self {
            adopter: application
                .ui
                .global::<ui::DifficulitySelectAdopter>()
                .as_weak(),
            nav_controller: application.nav_controller.clone(),
            projector_transport: application.repositories.projector.clone(),
        }
    }
}

impl NavDestination<NavRoute> for DifficultySelectDestination {
    fn load(&self, route: &NavRoute) {
        debug!("loading DifficultySelectViewModel");
        let NavRoute::DifficulitySelect = route else {
            panic!("matched variant should be given");
        };

        let adopter = self.adopter.unwrap();

        let viewmodel = Rc::new(DifficulitySelectViewModel::new(
            self.nav_controller.clone(),
            DifficulitySelectState::new(&adopter),
            self.projector_transport.clone(),
        ));

        macro_rules! cb {
            ($name: ident) => {
                bind_callback!(adopter, viewmodel, $name);
            };
        }

        cb!(start_game);
        adopter.on_select_difficulity({
            let viewmodel = Rc::clone(&viewmodel);
            move |d| {
                viewmodel.on_select_difficulity(d);
            }
        });
    }

    fn route(&self) -> NavRouteKind {
        NavRouteKind::DifficulitySelect
    }
}
