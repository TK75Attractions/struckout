use std::time::Duration;

use crate::{
    Application, Config, NavController,
    data::game_master::GameMasterClient,
    presentation::{
        connecting::ConnectingDestination, connection_failed::ConnectionFailedDestination,
        difficulity_select::DifficultySelectDestination, fallback::FallbackDestination,
        name_input::NameInputDestination, playing::PlayingDestination, ranking::RankingDestination,
        score::ScoreDestination, start::StartScreenDestination,
    },
    ui::NavRoute,
};
use stern::nav::NavHost;
use struckout_proto::game_master_service_client::GameMasterServiceClient;
use tokio::time::timeout;
use tracing::{debug, warn};

/// Defines `XxxViewModelRc` which wraps `XxxViewModel`.
///
/// `XxxViewModelRc::new()` registers viewmodel to the adopter
/// by calling [`stern::GlobalExt::register_viewmodel()`]).
///
/// `vm` is the name of viewmodel type (e.g. XxxViewModel)/
macro_rules! viewmodel_rc {
    ($vm:ident, $adopter:ty) => {
        pastey::paste! {
            #[allow(unused_imports)]
            use slint::ComponentHandle as _;
            #[allow(unused_imports)]
            use stern::GlobalExt as _;

            #[derive(derive_more::Deref)]
            struct [<$vm Rc>](std::rc::Rc<std::cell::RefCell<$vm>>);

            impl [<$vm Rc>] {
                fn new(application: &Application) -> Self {
                    let this = std::rc::Rc::new(std::cell::RefCell::new($vm::new(application)));
                    application.ui.global::<crate::ui::$adopter>()
                        .register_viewmodel(std::rc::Rc::clone(&this));

                    Self(this)
                }
            }
        }
    };
}

pub mod connecting;
pub mod connection_failed;
pub mod difficulity_select;
pub mod fallback;
pub mod name_input;
pub mod playing;
pub mod ranking;
pub mod score;
pub mod start;

/// Connects to game-master's gRPC server.
///
/// When succeeded, it navigates to `StartScreen` and returns connected [`GameMasterClient`].
/// When failed, it navigates to `ConnectionFailedScreen` with error message.
async fn connect_to_game_master(
    nc: NavController,
    config: &Config,
) -> Result<GameMasterClient<GameMasterServiceClient<tonic::transport::Channel>>, ()> {
    match timeout(
        Duration::from_secs(5),
        GameMasterClient::connect(&config.server_addr, config.machine_id.into()),
    )
    .await
    {
        Ok(Ok(v)) => {
            nc.navigate(NavRoute::Start);
            Ok(v)
        }
        Ok(Err(e)) => {
            nc.navigate(NavRoute::ConnectionFailed(format!(
                "game-masterへの接続に失敗しました: {}",
                e
            )));
            Err(())
        }
        Err(_) => {
            nc.navigate(NavRoute::ConnectionFailed(
                "タイムアウトしました".to_string(),
            ));
            Err(())
        }
    }
}

/// Initializes [`WorkerThread`]'s context. Note that since the function calls [`slint::spawn_local()`] to await async functions,
/// the process actually starts after slint's event loop has started. (e.g. by [`AppWindow::run()`])
///
/// [AppWindow::run]: crate::ui::AppWindow::run
pub fn init_worker_context(application: &Application) {
    let nc = application.nav_controller.clone();
    let mut worker = application.worker.clone();
    let config = application.config.clone();
    slint::spawn_local(async move {
        debug!("initializing worker context");
        let game_master = match connect_to_game_master(nc, &config).await {
            Ok(v) => v,
            Err(_) => {
                warn!("failed to connect to game-master. user can retry it.");
                return;
            }
        };

        {
            let cx = worker.context();
            let guard = cx.write();
            guard
                .game_master
                .set(game_master)
                .expect("this should be a first successful attempt to connect to game-master");
        }
        debug!("worker context initialized successfully");
    })
    .unwrap();
}

/// Registers each [`NavDestination`][crate::nav::NavDestination]s at [`NavHost`].
pub fn attach_navhost(application: &Application) {
    NavHost::builder(application.nav_controller.clone())
        .register(StartScreenDestination::new(&application))
        .register(NameInputDestination::new(&application))
        .register(DifficultySelectDestination::new(&application))
        .register(FallbackDestination::new(&application))
        .register(ConnectionFailedDestination::new(&application))
        .register(PlayingDestination::new(&application))
        .register(ScoreDestination::new(&application))
        .register(ConnectingDestination::new(&application))
        .register(RankingDestination::new(&application))
        .finish()
        .expect("failed to build NavHost");
}

#[cfg(test)]
mod tests {}
