use std::rc::Rc;

use crate::{
    Application, Config, Context, NavController,
    presentation::connect_to_game_master,
    ui::{self, ConnectionFailedStates, ConnectionFailedViewModelTrait, NavRoute, NavRouteKind},
};
use slint::{ComponentHandle, Global, ToSharedString};
use stern::{GlobalExt, WorkerThread, nav::NavDestination};
use tracing::{debug, warn};

viewmodel_rc!(ConnectionFailedViewModel, ConnectionFailedAdopter);

struct ConnectionFailedViewModel {
    nav_controller: NavController,
    config: Rc<Config>,
    worker: WorkerThread<Context>,
    state: ConnectionFailedStates,
}

impl ConnectionFailedViewModel {
    fn new(application: &Application) -> Self {
        Self {
            nav_controller: application.nav_controller.clone(),
            config: application.config.clone(),
            worker: application.worker.clone(),
            state: ConnectionFailedStates::new(
                application
                    .ui
                    .global::<ui::ConnectionFailedAdopter>()
                    .as_weak(),
            ),
        }
    }
}

impl ConnectionFailedViewModelTrait for ConnectionFailedViewModel {
    fn on_retry_connection(&mut self) {
        let nc = self.nav_controller.clone();
        let config = self.config.clone();
        let mut worker = self.worker.clone();
        slint::spawn_local(async move {
            debug!("retrying connection to game-master");
            let game_master = match connect_to_game_master(nc, &config).await{
                Ok(v) => v,
                Err(_) =>{
                    warn!("failed to connect to game-master. user can retry it.");
                    return;
                }
            };
            {
                let cx = worker.context();
                let guard = cx.write();
                guard.game_master.set(game_master).expect("this should be a first successful attempt to connect to game-master");
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
    fn load(&self, route: &NavRoute) {
        debug!("loading ConnectionFailedViewModel");

        let NavRoute::ConnectionFailed(msg) = route else {
            panic!("matched variant should be given");
        };

        self.viewmodel
            .0
            .borrow()
            .state
            .error_msg
            .set(msg.to_shared_string());
    }

    fn route(&self) -> NavRouteKind {
        NavRouteKind::ConnectionFailed
    }
}
