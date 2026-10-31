use std::sync::Arc;

use slint::{ComponentHandle, Global, ToSharedString};
use stern::{
    WorkerThread,
    nav::NavDestination,
    worker::{ForegroundExecutor, SlintExecutor},
};
use touchpanel_ui::{
    ConnectionFailedPropertyMappers, ConnectionFailedStates, ConnectionFailedViewModelTrait,
    NavRoute, NavRouteKind,
};
use tracing::{debug, warn};

use crate::{
    Application, Config, Context, NavController,
    presentation::{connect_to_game_master, initial_route, observe_session},
};

touchpanel_ui::define_connection_failed_mapper! {}

viewmodel_rc!(
    ConnectionFailedViewModel<SlintExecutor>,
    ConnectionFailedAdopter
);

struct ConnectionFailedViewModel<E> {
    nav_controller: NavController,
    config: Arc<Config>,
    worker: WorkerThread<Context, E>,
    state: ConnectionFailedStates<Mapper>,
}

impl ConnectionFailedViewModel<SlintExecutor> {
    fn new(application: &Application) -> Self {
        Self {
            nav_controller: application.nav_controller.clone(),
            config: application.config.clone(),
            worker: application.worker.clone(),
            state: ConnectionFailedStates::<Mapper>::new(
                application
                    .ui
                    .global::<touchpanel_ui::ConnectionFailedAdopter>()
                    .as_weak(),
            ),
        }
    }
}

impl<E> ConnectionFailedViewModelTrait for ConnectionFailedViewModel<E>
where
    E: ForegroundExecutor + Clone + 'static,
{
    fn on_retry_connection(&mut self) {
        let nc = self.nav_controller.clone();
        let config = self.config.clone();
        let worker = self.worker.clone();
        slint::spawn_local(async move {
            debug!("retrying connection to game-master");
            let simple_mode = config.simple_mode;
            let game_master = match connect_to_game_master(&worker, nc.clone(), config).await{
                Ok(v) => v,
                Err(_) =>{
                    warn!("failed to connect to game-master. user can retry it.");
                    return;
                }
            };
            {
                let cx = worker.background_executor().context();
                cx.game_master.set(game_master).expect("this should be a first successful attempt to connect to game-master");
            }
            observe_session(&worker, nc.clone());
            match initial_route(&worker, simple_mode).await {
                Ok(route) => nc.navigate(route),
                Err(error) => nc.navigate(NavRoute::Fallback(format!(
                    "シンプルモードの開始に失敗しました: {error}"
                ))),
            }
            debug!("connection retry to game-master succeeds and initialized worker context with game-master client");
        })
        .unwrap();
    }
}

pub struct ConnectionFailedDestination {
    viewmodel: ConnectionFailedViewModelRc,
}

impl ConnectionFailedDestination {
    pub fn new(application: &Application) -> Self {
        Self {
            viewmodel: ConnectionFailedViewModelRc::new(application),
        }
    }
}

impl NavDestination<NavRoute> for ConnectionFailedDestination {
    fn load(&mut self, route: &NavRoute) {
        debug!("loading ConnectionFailedViewModel");

        let NavRoute::ConnectionFailed(msg) = route else {
            panic!("matched variant should be given");
        };

        self.viewmodel
            .borrow()
            .state
            .error_msg
            .set(msg.to_shared_string());
    }

    fn route(&self) -> NavRouteKind {
        NavRouteKind::ConnectionFailed
    }
}
