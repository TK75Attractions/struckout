use crate::{
    Application,
    data::projector::{BindError, ConnectError},
    presentation::{
        connecting::ConnectingDestination, connection_failed::ConnectionFailedDestination,
        difficulity_select::DifficultySelectDestination, fallback::FallbackDestination,
        name_input::NameInputDestination, playing::PlayingDestination, ranking::RankingDestination,
        score::ScoreDestination, start::StartScreenDestination,
    },
    ui::NavRoute,
};
use slint_fw::nav::NavHost;
use tracing::{debug, info};

/// Defines `XxxViewModelRc` which wraps `XxxViewModel`.
///
/// `XxxViewModelRc::new()` registers viewmodel to the adopter
/// by calling [`slint_fw::GlobalExt::register_viewmodel()`]).
///
/// `vm` is the name of viewmodel type (e.g. XxxViewModel)/
macro_rules! viewmodel_rc {
    ($vm:ident, $adopter:ty) => {
        pastey::paste! {
            #[allow(unused_imports)]
            use slint::ComponentHandle as _;
            #[allow(unused_imports)]
            use slint_fw::GlobalExt as _;

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

pub fn init_connection(application: &Application) {
    debug!("initializing connection");

    let transport = application.repositories.projector.clone();
    let transport_clone = application.repositories.projector.clone();

    let nc = application.nav_controller.clone();

    slint::spawn_local(async move {
        let guard = transport.borrow_mut();
        let res = guard.bind().await;

        match res {
            Ok(()) => {
                info!("successfully bound port for TCP");
            }
            Err(BindError::AlreadyBound) => panic!("this should be first attempt to bind port"),
            Err(BindError::Other(e)) => {
                nc.navigate(NavRoute::Fallback(format!("failed to bind port: {}", e)));
                return;
            }
        }

        info!("connecting to projector");
        let res = guard.connect().await;
        match res {
            Ok(()) => {
                info!("connection succeeds");
                nc.navigate(NavRoute::Start);
            }
            Err(ConnectError::PortNotBound) => panic!("bound just before"),
            Err(ConnectError::Tcp(e)) => {
                nc.navigate(NavRoute::ConnectionFailed(format!(
                    "failed to accept connection: {}",
                    e
                )));
            }
            Err(ConnectError::Timeout(_)) => {
                nc.navigate(NavRoute::ConnectionFailed(
                    "タイムアウトしました".to_string(),
                ));
            }
            _ => todo!(),
        }
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
