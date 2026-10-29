use clap::Parser;
use slint::ComponentHandle;
use slint::winit_030::{WinitWindowAccessor, winit::window::Fullscreen};
use std::{
    cell::RefCell,
    num::NonZeroUsize,
    rc::Rc,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use stern::{WorkerThread, worker::SlintExecutor};
use tracing::info;

use touchpanel_ui::NavRoute;

use crate::{
    data::GameMasterClient,
    presentation::{attach_navhost, init_worker_context},
};

pub mod data;
mod keyboard;
pub mod presentation;

// provided by build.rs
const GAME_MASTER_GRPC_PORT: &str = env!("TOUCHPANEL_GAME_MASTER_GRPC_PORT");

type NavController = stern::nav::NavController<NavRoute>;

pub struct Application {
    nav_controller: NavController,
    ui: touchpanel_ui::AppWindow,
    #[allow(dead_code)] // チャンネルを生存させるために必要
    pub worker: WorkerThread<Context, SlintExecutor>,
    config: Arc<Config>,
}

/// Context of [`WorkerThread`]. i.e., state holded in tokio threads.
#[derive(Debug)]
pub struct Context {
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
#[command(version = version())]
struct Cli {
    #[arg(
        short = 'a',
        long = "address",
        help = "the address of game-master's gRPC server"
    )]
    server_addr: String,
    #[arg(short = 'm', long = "machine-id", help = "the id of this machine")]
    machine_id: u32,
    #[arg(
        long,
        value_name = "N",
        help = "the display to show on, counted from 1 in the order winit lists them (may differ from Unity's and Windows' numbers); the primary display if omitted"
    )]
    monitor: Option<NonZeroUsize>,
}

pub fn version() -> String {
    version_option().unwrap_or("no version".to_string())
}

pub fn version_option() -> Option<String> {
    if let Some(version) = option_env!("ST_VERSION") {
        let Some(short_hash) = option_env!("ST_COMMIT_SHORT_HASH") else {
            unreachable!("ST_COMMIT_SHORT_HASH should always exist when ST_VERSION is provided")
        };
        Some(format!("{} ({})", version, short_hash))
    } else {
        None
    }
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

pub fn run_main(shutdown_requested: Arc<AtomicBool>) -> anyhow::Result<()> {
    let cli = Cli::parse();
    let monitor = cli.monitor;
    if monitor.is_some() {
        slint::BackendSelector::new()
            .backend_name("winit".into())
            .select()?;
    }

    let config = Arc::new(Config::from_cli(cli));

    let ui = touchpanel_ui::AppWindow::new()?;

    let startup_error = Rc::new(RefCell::new(None));
    if let Some(monitor) = monitor {
        let ui = ui.as_weak();
        let startup_error = Rc::clone(&startup_error);
        // Native windows are created only once the event loop is active.
        slint::spawn_local(async move {
            let Some(ui) = ui.upgrade() else {
                return;
            };
            if let Err(error) = configure_monitor(ui.window(), monitor).await {
                *startup_error.borrow_mut() = Some(error);
                slint::quit_event_loop().expect("Slint event loop should be running");
            }
        })?;
    }

    let nav_controller = NavController::new(NavRoute::Connecting, {
        let ui = ui.as_weak();

        move |route| {
            let ui = ui.unwrap();
            ui.set_nav_route(route);
        }
    });
    ui.on_click_backbutton({
        let nc = nav_controller.clone();
        move || {
            nc.move_back();
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
    let shutdown_timer = slint::Timer::default();
    shutdown_timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(50),
        move || {
            if shutdown_requested.swap(false, Ordering::AcqRel) {
                slint::quit_event_loop().expect("Slint event loop should be running");
            }
        },
    );
    application.ui.run()?;
    if let Some(error) = startup_error.take() {
        return Err(error);
    }
    Ok(())
}

async fn configure_monitor(window: &slint::Window, number: NonZeroUsize) -> anyhow::Result<()> {
    let window = window.winit_window().await?;
    let monitors: Vec<_> = window.available_monitors().collect();
    for (index, monitor) in monitors.iter().enumerate() {
        info!(number = index + 1, name = ?monitor.name(), position = ?monitor.position(), "available display");
    }
    let monitor = monitors.get(number.get() - 1).ok_or_else(|| {
        anyhow::anyhow!(
            "ディスプレイ {} がありません。{} 台見つかりました",
            number,
            monitors.len()
        )
    })?;
    window.set_fullscreen(Some(Fullscreen::Borderless(Some(monitor.clone()))));
    Ok(())
}
