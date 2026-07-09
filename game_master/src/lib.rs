use slint::ComponentHandle;

use crate::{nav::NavController, viewmodels::start_screen};

mod ui {
    slint::include_modules!();
}

mod data;
mod nav;
mod state_ext;
mod viewmodels;

struct Application {
    nav_controller: NavController,
    ui: ui::AppWindow,
}

pub fn run_main() {
    let ui = ui::AppWindow::new().unwrap();

    let nav_controller = NavController::new(ui::NavRoute::Start, {
        let ui = ui.as_weak();

        move |route| {
            let ui = ui.unwrap();
            ui.set_nav_route(route);
        }
    });

    let application = Application { nav_controller, ui };

    start_screen::init(&application);

    application.ui.run().unwrap();
}
