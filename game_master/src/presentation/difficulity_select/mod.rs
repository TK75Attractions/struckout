use slint::{ComponentHandle, Global};

use crate::{Application, nav::NavController, ui};

pub struct DifficulitySelectViewModel {
    nav_controller: NavController,
    state: DifficulitySelectState,
}

impl DifficulitySelectViewModel {
    fn new(nav_controller: NavController, state: DifficulitySelectState) -> Self {
        Self {
            nav_controller,
            state,
        }
    }

    fn on_select_difficulity() {
        todo!()
    }

    fn on_start_game() {
        todo!()
    }
}

state_struct!(
    DifficulitySelect,
    selected_difficulity => ui::Difficulity
);

pub fn init(application: &Application) {
    let adopter = application.ui.global::<ui::DifficulitySelectAdopter>();
    let state = DifficulitySelectState::new(&adopter);
    let viewmodel = DifficulitySelectViewModel::new(application.nav_controller.clone(), state);

    todo!()
}
