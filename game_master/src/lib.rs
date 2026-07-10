use std::{cell::OnceCell, rc::Rc};

use slint::ComponentHandle;
use sqlx::sqlite::SqlitePoolOptions;
use tokio::sync::oneshot;

use crate::{
    nav::NavController,
    player_repository::PlayerRepository,
    viewmodels::start_screen::{self, StartScreenViewModel},
    worker::WorkerThread,
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

const SQLITE_DEFAULT_URL: &str = "sqlite:///home/taichi765/.config/struckout/game_master_dev.db";

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
    /// チャンネルを生存させるために必要
    #[allow(dead_code)]
    pub worker: WorkerThread,
}

impl RepositoryOwner {
    fn new() -> Self {
        let worker = WorkerThread::new();
        let (tx, rx) = oneshot::channel();
        worker.spawn(async move {
            let res = SqlitePoolOptions::new()
                .max_connections(5)
                .connect(SQLITE_DEFAULT_URL)
                .await;
            tx.send(res).unwrap();
        });

        // FIXME: 普通にブロックする. slint::spawn_local()など
        let pool = rx
            .blocking_recv()
            .unwrap()
            .expect("failed to connec to database");
        Self {
            player: Rc::new(PlayerRepository::new(pool, &worker)),
            worker,
        }
    }
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

    let application = Application {
        nav_controller,
        ui,
        viewmodels: ViewModelOwner::new(),
        repositories: RepositoryOwner::new(),
    };

    start_screen::init(&application);
    name_input::init(&application);

    application.ui.run().unwrap();
}
