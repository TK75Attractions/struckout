use slint::ComponentHandle;

mod ui {
    slint::include_modules!();
}

pub fn run_main() {
    let ui = ui::AppWindow::new().unwrap();

    ui.run().unwrap();
}
