use std::rc::Rc;

use slint::ComponentHandle;
use tracing::{debug, trace};

use crate::{Application, nav::NavController, ui};

#[derive(Debug)]
pub struct StartScreenViewModel {
    nav_controller: NavController,
}

impl StartScreenViewModel {
    pub fn new(nav_controller: NavController) -> Self {
        Self { nav_controller }
    }

    pub fn on_click(&self) {
        trace!("StartScreen::on_click()");
        self.nav_controller.navigate(ui::NavRoute::NameInput);
    }
}

pub fn init<PT>(application: &Application<PT>) {
    debug!("initializing StartScreen");
    let adopter = application.ui.global::<ui::StartScreenAdopter>();

    let viewmodel = Rc::new(StartScreenViewModel::new(
        application.nav_controller.clone(),
    ));

    adopter.on_click({
        let viewmodel = Rc::clone(&viewmodel);
        move || {
            viewmodel.on_click();
        }
    });

    application.viewmodels.start_screen.set(viewmodel).unwrap();
}
