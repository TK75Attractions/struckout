use slint::ComponentHandle;
use sqlx::sqlite::SqlitePoolOptions;
use std::cell::RefCell;
use std::{cell::OnceCell, rc::Rc};
use tokio::sync::oneshot;

use crate::{
    data::{player::PlayerRepository, projector::ProjectorConnectionImpl},
    nav::NavController,
    presentation::start::StartScreenViewModel,
    worker::WorkerThread,
};

mod ui {
    slint::include_modules!();
}

mod data;
mod nav;
mod presentation;
mod state_ext;
mod worker;

const SQLITE_DEFAULT_URL: &str = "sqlite:///home/taichi765/.config/struckout/game_master_dev.db";

struct Application<PT> {
    nav_controller: NavController,
    ui: ui::AppWindow,
    viewmodels: ViewModelOwner,
    repositories: RepositoryOwner<PT>,
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
struct RepositoryOwner<PT> {
    pub player: Rc<PlayerRepository>,
    pub projector: Rc<RefCell<PT>>,
    #[allow(dead_code)] // チャンネルを生存させるために必要
    pub worker: WorkerThread,
}

impl RepositoryOwner<ProjectorConnectionImpl> {
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
            projector: todo!(),
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

    presentation::init(&application);

    application.ui.run().unwrap();
}
