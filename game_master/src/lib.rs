use std::{cell::OnceCell, rc::Rc};

use slint::ComponentHandle;
use sqlx::{Pool, Sqlite, sqlite::SqlitePoolOptions};

use crate::{
    nav::NavController,
    player_repository::PlayerRepository,
    viewmodels::start_screen::{self, StartScreenViewModel},
};

mod ui {
    slint::include_modules!();
}

mod data;
mod name_input;
mod nav;
mod player_repository;
mod state_ext;
mod viewmodels;
mod worker;

const SQLITE_DEFAULT_URL: &str = "sqlite:///home/taichi765/.config/struckout/dev.db";

struct Application {
    nav_controller: NavController,
    ui: ui::AppWindow,
    viewmodels: ViewModelOwner,
    repositories: RepositoryOwner,
}

struct ViewModelOwner {
    pub start_screen: OnceCell<Rc<StartScreenViewModel>>,
}

impl ViewModelOwner {
    fn new() -> Self {
        Self {
            start_screen: OnceCell::new(),
        }
    }
}

/// Container for repositories.
struct RepositoryOwner {
    pub player: Rc<PlayerRepository>,
}

impl RepositoryOwner {
    fn new(pool: Pool<Sqlite>) -> Self {
        Self {
            player: Rc::new(PlayerRepository::new(pool)),
        }
    }
}

pub async fn run_main() {
    let ui = ui::AppWindow::new().unwrap();

    let nav_controller = NavController::new(ui::NavRoute::Start, {
        let ui = ui.as_weak();

        move |route| {
            let ui = ui.unwrap();
            ui.set_nav_route(route);
        }
    });

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(SQLITE_DEFAULT_URL)
        .await
        .expect("failed to connect to database");

    let application = Application {
        nav_controller,
        ui,
        viewmodels: ViewModelOwner::new(),
        repositories: RepositoryOwner::new(pool.clone()),
    };

    start_screen::init(&application);
    name_input::init(&application);

    application.ui.run().unwrap();
}
