use std::cell::RefCell;
use std::rc::Rc;

use slint::{ComponentHandle, Global, ToSharedString};

use crate::{
    Application, NavController,
    data::projector::{ProjectorTransport, StartGameError},
    ui::{self, DifficulitySelectStates, DifficulitySelectViewModelTrait, NavRoute, NavRouteKind},
};
use slint_fw::{GlobalExt as _, nav::NavDestination};
use tracing::{debug, error, trace};

viewmodel_rc!(DifficulitySelectViewModel, DifficulitySelectAdopter);

#[derive(Debug)]
struct DifficulitySelectViewModel {
    nav_controller: NavController,
    state: DifficulitySelectStates,
    projector_transport: Rc<RefCell<ProjectorTransport>>,
}

impl DifficulitySelectViewModel {
    fn new(application: &Application) -> Self {
        Self {
            nav_controller: application.nav_controller.clone(),
            state: DifficulitySelectStates::new(
                application
                    .ui
                    .global::<ui::DifficulitySelectAdopter>()
                    .as_weak(),
            ),
            projector_transport: application.repositories.projector.clone(),
        }
    }
}

impl DifficulitySelectViewModelTrait for DifficulitySelectViewModel {
    fn on_start_game(&mut self) {
        trace!("DifficulitySelectViewModel::on_start_game");

        let difficulty = self.state.selected_difficulity.get();
        let error_msg = self.state.error_msg.clone();
        let nc = self.nav_controller.clone();
        let trans = self.projector_transport.clone();
        slint::spawn_local(async move {
            let res = trans.borrow().start_game(difficulty).await;
            match res {
                Ok(()) => {
                    nc.navigate(NavRoute::Playing(difficulty));
                }
                Err(StartGameError::NotConnected) => panic!("should be already connected"),
                Err(StartGameError::Tcp(e)) => {
                    error!(?e, "failed to send StartGame message to projector");
                    error_msg.set("ネットワーク接続に失敗しました".to_shared_string());
                }
            }
        })
        .unwrap();
    }

    fn on_select_difficulity(&mut self, val: ui::Difficulity) {
        trace!(?val, "DifficulitySelectViewModel::on_select_difficulity");
        self.state.selected_difficulity.set(val);
    }
}

pub struct DifficultySelectDestination {
    #[allow(unused)] // just for initialize viewmodel
    viewmodel: DifficulitySelectViewModelRc,
}

impl DifficultySelectDestination {
    pub fn new(application: &Application) -> Self {
        Self {
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
        // do nothing because `NavRoute::DifficulitySelect` has no extra arguments
    }

    fn route(&self) -> NavRouteKind {
        NavRouteKind::DifficulitySelect
    }
}
