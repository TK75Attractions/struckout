#![allow(dead_code, unused_variables)]

use clap::Parser;
use slint::ComponentHandle;
use std::sync::{Arc, OnceLock};
use stern::WorkerThread;
use tracing::info;

use touchpanel_ui::NavRoute;

use crate::{
    data::GameMasterClient,
    presentation::{attach_navhost, init_worker_context},
};

mod data;
mod presentation;
mod state_ext;

const GAME_MASTER_GRPC_PORT: &str = env!("TOUCHPANEL_GAME_MASTER_GRPC_PORT");

type NavController = stern::nav::NavController<NavRoute>;
type NavHost = stern::nav::NavHost<NavRoute>;
type NavHostBuilder = stern::nav::NavHostBuilder<NavRoute>;
type NavHostBuilderError = stern::nav::NavHostBuilderError<NavRoute>;

struct Application {
    nav_controller: NavController,
    ui: touchpanel_ui::AppWindow,
    #[allow(dead_code)] // チャンネルを生存させるために必要
    pub worker: WorkerThread<Context>,
    config: Arc<Config>,
}

/// Context of [`WorkerThread`]. i.e., state holded in tokio threads.
#[derive(Debug)]
struct Context {
    game_master: OnceLock<GameMasterClient>,
}

impl Context {
    /// Constructs empty context.
    fn new_empty() -> Self {
        Self {
            game_master: OnceLock::new(),
        }
    }
}

/// CLI arguments.
#[derive(Parser)]
struct Cli {
    #[arg(
        short = 'a',
        long = "address",
        help = "the address of game-master's gRPC server"
    )]
    server_addr: String,
    #[arg(short = 'm', help = "the id of this machine")]
    machine_id: u32,
}

/// Application configs.
///
/// The fields are similar with [`Cli`] for now, but in future it's possible to
/// read config from a YAML file and merge it with CLI args, so it's better to define
/// each struct.
struct Config {
    /// The address of game-master's gRPC server.
    server_addr: String,
    /// The id of this machine.
    machine_id: u32,
}

impl Config {
    /// Constructs [`Config`] from CLI arguments.
    fn from_cli(cli: Cli) -> Self {
        Self {
            server_addr: cli.server_addr,
            machine_id: cli.machine_id,
        }
    }
}

pub fn run_main() {
    let cli = Cli::parse();
    let config = Arc::new(Config::from_cli(cli));

    let ui = touchpanel_ui::AppWindow::new().unwrap();

    let nav_controller = NavController::new(NavRoute::Connecting, {
        let ui = ui.as_weak();

        move |route| {
            let ui = ui.unwrap();
            ui.set_nav_route(route.into());
        }
    });

    let cx = Context::new_empty();
    let worker = WorkerThread::new(cx);

    let application = Application {
        nav_controller,
        ui,
        worker,
        config,
    };

    attach_navhost(&application);

    // NavHostを初期化したあとで
    init_worker_context(&application);

    info!("starting event loop");
    application.ui.run().unwrap();
}
