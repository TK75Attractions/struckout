use std::cell::RefCell;
use std::rc::Rc;

use slint::{ComponentHandle, Global, SharedString, ToSharedString};

use crate::{
    Application, NavController,
    data::projector::{ProjectorTransport, StartGameError},
    ui::{self, DifficulitySelectStates, DifficulitySelectViewModelTrait, NavRoute, NavRouteKind},
};
use slint_fw::{GlobalExt as _, nav::NavDestination};
use tracing::{debug, error, trace};

struct DifficulitySelectViewModelRc(Rc<RefCell<DifficulitySelectViewModel>>);

impl DifficulitySelectViewModelRc {
    fn new(application: &Application) -> Self {
        let this = Rc::new(RefCell::new(DifficulitySelectViewModel {
            nav_controller: application.nav_controller.clone(),
            state: DifficulitySelectStates::new(
                application
                    .ui
                    .global::<ui::DifficulitySelectAdopter>()
                    .as_weak(),
            ),
            projector_transport: application.repositories.projector.clone(),
        }));
        application
            .ui
            .global::<ui::DifficulitySelectAdopter>()
            .register_viewmodel(Rc::clone(&this));

        Self(this)
    }
}

#[derive(Debug)]
struct DifficulitySelectViewModel {
    nav_controller: NavController,
    state: DifficulitySelectStates,
    projector_transport: Rc<RefCell<ProjectorTransport>>,
}

impl DifficulitySelectViewModelTrait for DifficulitySelectViewModel {
    fn on_start_game(&mut self) {
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

    fn on_select_difficulity(&mut self, val: ui::Difficulity) {
        trace!(?val, "DifficulitySelectViewModel::on_select_difficulity");
        self.state.selected_difficulity.set(val);
    }
}

pub struct DifficultySelectDestination {
    adopter: slint::Weak<ui::DifficulitySelectAdopter<'static>>,
    nav_controller: NavController,
    projector_transport: Rc<RefCell<ProjectorTransport>>,
    viewmodel: DifficulitySelectViewModelRc,
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
            viewmodel: DifficulitySelectViewModelRc::new(application),
        }
    }
}

impl NavDestination<NavRoute> for DifficultySelectDestination {
    fn load(&self, route: &NavRoute) {
        debug!("loading DifficultySelectViewModel");
        let NavRoute::DifficulitySelect = route else {
            panic!("matched variant should be given");
        };

        todo!();
    }

    fn route(&self) -> NavRouteKind {
        NavRouteKind::DifficulitySelect
    }
}
