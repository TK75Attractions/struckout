use clap::Parser;
use serde::Deserialize;
use slint::ComponentHandle;
use slint::winit_030::{WinitWindowAccessor, winit::window::Fullscreen};
use std::{
    cell::RefCell,
    fs, io,
    num::NonZeroUsize,
    path::Path,
    rc::Rc,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use stern::{WorkerThread, worker::SlintExecutor};
use thiserror::Error;
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
    server_addr: Option<String>,
    #[arg(short = 'm', long = "machine-id", help = "the id of this machine")]
    machine_id: Option<u32>,
    #[arg(
        long,
        value_name = "N",
        help = "the display to show on, counted from 1 in the order winit lists them (may differ from Unity's and Windows' numbers); the primary display if omitted"
    )]
    monitor: Option<NonZeroUsize>,
    #[arg(long, help = "skip mode selection and start a single-player session")]
    disable_multiplay: bool,
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

/// Values read from the optional configuration file.
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct FileConfig {
    address: Option<String>,
    machine_id: Option<u32>,
    #[serde(default)]
    disable_multiplay: bool,
    monitor: Option<NonZeroUsize>,
}

#[derive(Debug, Error)]
enum LoadConfigError {
    #[error("設定ファイル {path} を読めません: {source}", path = .path.display())]
    Io {
        path: std::path::PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("設定ファイル {path} を解析できません: {source}", path = .path.display())]
    Parse {
        path: std::path::PathBuf,
        #[source]
        source: toml::de::Error,
    },
}

#[derive(Debug, Error)]
enum ResolveConfigError {
    #[error("--{0} または設定ファイルの {0} が必要です")]
    Missing(&'static str),
}

impl FileConfig {
    /// Reads the optional file so existing CLI-only installations keep working.
    fn from_file(path: &Path) -> Result<Self, LoadConfigError> {
        let content = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(source) => {
                return Err(LoadConfigError::Io {
                    path: path.into(),
                    source,
                });
            }
        };
        toml::from_str(&content).map_err(|source| LoadConfigError::Parse {
            path: path.into(),
            source,
        })
    }
}

/// Application config shared by the UI and the game master client.
struct Config {
    /// The address of game-master's gRPC server.
    server_addr: String,
    /// The id of this machine.
    machine_id: u32,
    /// Whether the mode selection screen is available.
    multiplay_availability: touchpanel_ui::MultiPlayAvailability,
    /// The display to show on, from 1 in winit's order. `None` for the primary display.
    monitor: Option<NonZeroUsize>,
}

impl Config {
    /// Combines file values with explicit CLI values, which have priority.
    fn from_sources(file: FileConfig, cli: Cli) -> Result<Self, ResolveConfigError> {
        Ok(Self {
            server_addr: cli
                .server_addr
                .or(file.address)
                .ok_or(ResolveConfigError::Missing("address"))?,
            machine_id: cli
                .machine_id
                .or(file.machine_id)
                .ok_or(ResolveConfigError::Missing("machine-id"))?,
            multiplay_availability: if cli.disable_multiplay || file.disable_multiplay {
                touchpanel_ui::MultiPlayAvailability::Disabled
            } else {
                touchpanel_ui::MultiPlayAvailability::Enabled
            },
            monitor: cli.monitor.or(file.monitor),
        })
    }
}

pub fn run_main(shutdown_requested: Arc<AtomicBool>) -> anyhow::Result<()> {
    let cli = Cli::parse();
    let file = match dirs::config_dir() {
        Some(dir) => FileConfig::from_file(&dir.join("struckout/touchpanel/config.toml"))?,
        None => FileConfig::default(),
    };
    let config = Arc::new(Config::from_sources(file, cli)?);

    let ui = touchpanel_ui::AppWindow::new()?;
    ui.set_mode_selection_enabled(
        config.multiplay_availability == touchpanel_ui::MultiPlayAvailability::Enabled,
    );

    let startup_error = Rc::new(RefCell::new(None));
    if let Some(monitor) = config.monitor {
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

/// Shows `window` full screen on the `number`th display.
///
/// The backend is left to Slint's usual selection (e.g. `SLINT_BACKEND`), which is not always winit.
/// Only the winit backend can choose a display, so on another backend this fails rather than
/// silently showing on the primary display.
async fn configure_monitor(window: &slint::Window, number: NonZeroUsize) -> anyhow::Result<()> {
    let window = window.winit_window().await.map_err(|error| {
        anyhow::anyhow!(
            "monitor は Slint の winit バックエンドでしか使えません。SLINT_BACKEND で別のバックエンドを選んでいないか確かめてください: {error}"
        )
    })?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_config_file_and_uses_cli_for_missing_values() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "address = '127.0.0.1'\ndisable-multiplay = true\n").unwrap();
        let cli = Cli::try_parse_from(["touchpanel", "--machine-id", "1"]).unwrap();
        let config = Config::from_sources(FileConfig::from_file(&path).unwrap(), cli).unwrap();
        assert_eq!(config.server_addr, "127.0.0.1");
        assert_eq!(config.machine_id, 1);
        assert_eq!(
            config.multiplay_availability,
            touchpanel_ui::MultiPlayAvailability::Disabled
        );
    }

    #[test]
    fn cli_overrides_config_file() {
        let file = toml::from_str("address = 'file'\nmachine-id = 1\n").unwrap();
        let cli = Cli::try_parse_from([
            "touchpanel",
            "--address",
            "cli",
            "--machine-id",
            "2",
            "--disable-multiplay",
        ])
        .unwrap();
        let config = Config::from_sources(file, cli).unwrap();
        assert_eq!(config.server_addr, "cli");
        assert_eq!(config.machine_id, 2);
        assert_eq!(
            config.multiplay_availability,
            touchpanel_ui::MultiPlayAvailability::Disabled
        );
    }

    #[test]
    fn config_file_can_supply_all_required_values() {
        let file = toml::from_str("address = '127.0.0.1'\nmachine-id = 1\n").unwrap();
        let cli = Cli::try_parse_from(["touchpanel"]).unwrap();
        let config = Config::from_sources(file, cli).unwrap();
        assert_eq!(config.server_addr, "127.0.0.1");
        assert_eq!(config.machine_id, 1);
        assert_eq!(
            config.multiplay_availability,
            touchpanel_ui::MultiPlayAvailability::Enabled
        );
    }

    #[test]
    fn monitor_comes_from_cli_or_config_file() {
        let file = toml::from_str(
            "address = 'a'
machine-id = 1
monitor = 2
",
        )
        .unwrap();
        let config =
            Config::from_sources(file, Cli::try_parse_from(["touchpanel"]).unwrap()).unwrap();
        assert_eq!(config.monitor, NonZeroUsize::new(2));

        let file = toml::from_str(
            "address = 'a'
machine-id = 1
monitor = 2
",
        )
        .unwrap();
        let cli = Cli::try_parse_from(["touchpanel", "--monitor", "3"]).unwrap();
        assert_eq!(
            Config::from_sources(file, cli).unwrap().monitor,
            NonZeroUsize::new(3)
        );

        assert!(
            toml::from_str::<FileConfig>(
                "monitor = 0
"
            )
            .is_err()
        );
    }

    #[test]
    fn missing_config_file_keeps_cli_only_startup() {
        let dir = tempfile::tempdir().unwrap();
        let file = FileConfig::from_file(&dir.path().join("config.toml")).unwrap();
        let cli = Cli::try_parse_from(["touchpanel", "-a", "127.0.0.1", "-m", "1"]).unwrap();
        assert!(Config::from_sources(file, cli).is_ok());
    }

    #[test]
    fn missing_required_values_and_invalid_file_are_errors() {
        let cli = Cli::try_parse_from(["touchpanel"]).unwrap();
        assert!(Config::from_sources(FileConfig::default(), cli).is_err());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "machine-id = 'wrong'\n").unwrap();
        assert!(matches!(
            FileConfig::from_file(&path),
            Err(LoadConfigError::Parse { .. })
        ));
    }
}
