use slint::ComponentHandle;
use sqlx::sqlite::SqlitePoolOptions;
use std::cell::RefCell;
use std::rc::Rc;
use tokio::sync::oneshot;
use tracing::info;

use crate::{
    data::{
        player::PlayerRepository,
        projector::{ProjectorTransport, ProjectorTransportImpl},
    },
    nav::NavController,
    presentation::{attach_navhost, init_connection},
    session::SessionManager,
    worker::WorkerThread,
};

mod ui {
    slint::include_modules!();
}

mod data;
mod nav;
mod presentation;
mod session;
mod state_ext;
mod worker;

const SQLITE_DEFAULT_URL: &str = "sqlite:///home/taichi765/.config/struckout/0716.db";

struct Application {
    nav_controller: NavController,
    ui: ui::AppWindow,
    repositories: RepositoryOwner,
    session_manager: Rc<RefCell<SessionManager>>,
}

/// Container for repositories.
struct RepositoryOwner {
    pub player: Rc<PlayerRepository>,
    pub projector: Rc<RefCell<ProjectorTransport>>,
    #[allow(dead_code)] // チャンネルを生存させるために必要
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
            projector: Rc::new(RefCell::new(ProjectorTransport::ProjectorTransportImpl(
                ProjectorTransportImpl::new(&worker),
            ))),
            worker,
        }
    }
}

pub fn run_main() {
    let ui = ui::AppWindow::new().unwrap();

    let nav_controller = NavController::new(ui::NavRoute::Connecting, {
        let ui = ui.as_weak();

        move |route| {
            let ui = ui.unwrap();
            ui.set_nav_route(route.into());
        }
    });
    let repositories = RepositoryOwner::new();
    let session_manager = Rc::new(RefCell::new(SessionManager::new(
        repositories.projector.clone(),
    )));

    let application = Application {
        nav_controller,
        ui,
        session_manager,
        repositories,
    };

    attach_navhost(&application);

    // NavHostを初期化したあとで
    init_connection(&application);

    info!("starting event loop");
    application.ui.run().unwrap();
}
