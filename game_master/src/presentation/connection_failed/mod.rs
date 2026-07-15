use std::{cell::RefCell, rc::Rc};

use crate::{
    Application,
    data::projector::{ConnectError, ProjectorConnection},
    nav::{NavController, NavDestination, NavRoute, NavRouteKind},
    ui,
};
use slint::{ComponentHandle, Global, SharedString, ToSharedString};
use tracing::debug;

state_struct!(ConnectionFailed, error_msg => SharedString);

struct ConnectionFailedViewModel<PT> {
    nav_controller: NavController,
    projector_transport: Rc<RefCell<PT>>,
    state: ConnectionFailedState,
}

impl<PT> ConnectionFailedViewModel<PT>
where
    PT: ProjectorConnection,
{
    fn on_retry_connection(&self) {
        self.projector_transport.borrow_mut().connect({
            let nav_controller = self.nav_controller.clone();
            let error_msg = self.state.error_msg.clone();
            move |res| match res {
                Ok(()) => {
                    nav_controller.navigate(NavRoute::Start); // TODO: プレイ中に接続が切れた時どうするか
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
            }
        });
    }
}

pub struct ConnectionFailedDestination<PT> {
    adopter: slint::Weak<ui::ConnectionFailedAdopter<'static>>,
    nav_controller: NavController,
    projector_transport: Rc<RefCell<PT>>,
}

impl<PT> ConnectionFailedDestination<PT> {
    pub fn new(application: &Application<PT>) -> Self {
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

impl<PT> NavDestination for ConnectionFailedDestination<PT>
where
    PT: ProjectorConnection + 'static,
{
    fn load(&self, route: &NavRoute) {
        debug!("loading ConnectionFailedViewModel");

        let NavRoute::ConnectionFailed(msg) = route else {
            panic!("matched variant should be given");
        };

        let adopter = self.adopter.unwrap();
        let viewmodel = Rc::new(ConnectionFailedViewModel {
            nav_controller: self.nav_controller.clone(),
            projector_transport: self.projector_transport.clone(),
            state: ConnectionFailedState::new(&adopter),
        });

        viewmodel.state.error_msg.set(msg.to_shared_string());

        bind_callback!(adopter, viewmodel, retry_connection);
    }

    fn route(&self) -> NavRouteKind {
        NavRouteKind::ConnectionFailed
    }
}
