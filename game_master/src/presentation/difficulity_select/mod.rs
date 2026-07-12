use std::cell::RefCell;
use std::rc::Rc;

use slint::{ComponentHandle, Global, SharedString, ToSharedString};

use crate::{
    Application,
    data::projector::{ProjectorConnection, StartGameError},
    nav::NavController,
    ui::{self, NavRoute},
};
use tracing::{error, trace};

pub struct DifficulitySelectViewModel<PT> {
    nav_controller: NavController,
    state: DifficulitySelectState,
    projector_transport: Rc<RefCell<PT>>,
}

impl<PT> DifficulitySelectViewModel<PT>
where
    PT: ProjectorConnection,
{
    fn new(
        nav_controller: NavController,
        state: DifficulitySelectState,
        projector_transport: Rc<RefCell<PT>>,
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
        let error_msg = self.state.error_msg.clone();
        let nav_controller = self.nav_controller.clone();
        self.projector_transport
            .borrow_mut()
            .start_game(difficulty, move |res| match res {
                Ok(()) => {
                    nav_controller.navigate(NavRoute::Playing);
                }
                Err(StartGameError::NotConnected) => panic!("should be already connected"),
                Err(StartGameError::Tcp(e)) => {
                    error!(?e, "failed to send StartGame message to projector");
                    error_msg.set("ネットワーク接続に失敗しました".to_shared_string());
                }
            });
    }
}

state_struct!(
    DifficulitySelect,
    selected_difficulity => ui::Difficulity,
    error_msg => SharedString
);

pub fn init<PT>(application: &Application<PT>)
where
    PT: ProjectorConnection + 'static,
{
    let adopter = application.ui.global::<ui::DifficulitySelectAdopter>();
    let state = DifficulitySelectState::new(&adopter);
    let viewmodel = Rc::new(DifficulitySelectViewModel::new(
        application.nav_controller.clone(),
        state,
        Rc::clone(&application.repositories.projector),
    ));

    macro_rules! cb {
        ($name:ident) => {
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
