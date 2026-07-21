use std::{cell::RefCell, rc::Rc};

use crate::{
    Application, NavController,
    data::projector::{ConnectError, ProjectorTransport},
    ui::{self, ConnectionFailedStates, ConnectionFailedViewModelTrait, NavRoute, NavRouteKind},
};
use slint::{ComponentHandle, Global, ToSharedString};
use slint_fw::{GlobalExt, nav::NavDestination};
use tracing::debug;

viewmodel_rc!(ConnectionFailedViewModel, ConnectionFailedAdopter);

struct ConnectionFailedViewModel {
    nav_controller: NavController,
    projector_transport: Rc<RefCell<ProjectorTransport>>,
    state: ConnectionFailedStates,
}

impl ConnectionFailedViewModel {
    fn new(application: &Application) -> Self {
        Self {
            nav_controller: application.nav_controller.clone(),
            projector_transport: application.repositories.projector.clone(),
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
        let trans = self.projector_transport.clone();
        let error_msg = self.state.error_msg.clone();
        slint::spawn_local(async move {
            let res = trans.borrow().connect().await;
            match res {
                Ok(()) => {
                    nc.navigate(NavRoute::Start); // TODO: プレイ中に接続が切れた時どうするか
                }
                Err(e) => match e {
                    ConnectError::AlreadyConnected
                    | ConnectError::PortNotBound
                    | ConnectError::AlreadyWaitingForConnection => {
                        panic!("status should not be {e:?} while showing ConnectionFailedScreen");
                    }
                    ConnectError::Timeout(_) => {
                        error_msg.set("タイムアウトしました".to_shared_string());
                    }
                    ConnectError::Tcp(e) => {
                        error_msg.set(format!("接続に失敗しました: {}", e).to_shared_string());
                    }
                },
            }
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
