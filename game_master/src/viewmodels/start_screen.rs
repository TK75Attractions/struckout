use slint::ComponentHandle;

use crate::{Application, nav::NavController, ui};

pub struct StartScreenViewModel {
    nav_controller: NavController,
}

impl StartScreenViewModel {
    pub fn new(application: &Application) -> Self {
        Self {
            nav_controller: application.nav_controller.clone(),
        }
    }

    pub fn on_click(&self) {
        self.nav_controller.navigate(ui::NavRoute::NameInput);
    }
}

pub fn init(application: &Application) {
    let adopter = application.ui.global::<ui::StartScreenAdopter>();

    let viewmodel = StartScreenViewModel::new(application);

    adopter.on_click(move || {
        viewmodel.on_click();
    });
}
