use std::{cell::RefCell, rc::Rc};

use crate::{
    Application, NavController,
    data::projector::{ConnectError, ProjectorTransport},
    ui::{self, ConnectionFailedStates, NavRoute, NavRouteKind},
};
use slint::{ComponentHandle, Global, SharedString, ToSharedString};
use slint_fw::nav::NavDestination;
use tracing::debug;

struct ConnectionFailedViewModel {
    nav_controller: NavController,
    projector_transport: Rc<RefCell<ProjectorTransport>>,
    state: ConnectionFailedStates,
}

impl ConnectionFailedViewModel {
    fn on_retry_connection(&self) {
        let nc = self.nav_controller.clone();
        let trans = self.projector_transport.clone();
        let error_msg = self.state.error_msg.clone();
        slint::spawn_local(async move {
            let res = trans.borrow_mut().connect().await;
            match res {
                Ok(()) => {
                    nc.navigate(NavRoute::Start); // TODO: プレイ中に接続が切れた時どうするか
                }
                Err(ConnectError::PortNotBound) => {
                    panic!("port should be always bound when ConnectionFailedScreen is shown")
                }
                Err(ConnectError::Timeout(_)) => {
                    error_msg.set("タイムアウトしました".to_shared_string());
                }
                Err(ConnectError::Tcp(e)) => {
                    error_msg.set(format!("接続に失敗しました: {}", e).to_shared_string());
                }
                _ => todo!(),
            }
        })
        .unwrap();
    }
}

pub struct ConnectionFailedDestination {
    adopter: slint::Weak<ui::ConnectionFailedAdopter<'static>>,
    nav_controller: NavController,
    projector_transport: Rc<RefCell<ProjectorTransport>>,
}

impl ConnectionFailedDestination {
    pub fn new(application: &Application) -> Self {
        Self {
            adopter: application
                .ui
                .global::<ui::ConnectionFailedAdopter>()
                .as_weak(),
            nav_controller: application.nav_controller.clone(),
            projector_transport: application.repositories.projector.clone(),
        }
    }
}

impl NavDestination<NavRoute> for ConnectionFailedDestination {
    fn load(&self, route: &NavRoute) {
        debug!("loading ConnectionFailedViewModel");

        let NavRoute::ConnectionFailed(msg) = route else {
            panic!("matched variant should be given");
        };

        let adopter = self.adopter.unwrap();
        let viewmodel = Rc::new(ConnectionFailedViewModel {
            nav_controller: self.nav_controller.clone(),
            projector_transport: self.projector_transport.clone(),
            state: ConnectionFailedStates::new(adopter.as_weak()),
        });

        viewmodel.state.error_msg.set(msg.to_shared_string());

        bind_callback!(adopter, viewmodel, retry_connection);
    }

    fn route(&self) -> NavRouteKind {
        NavRouteKind::ConnectionFailed
    }
}
