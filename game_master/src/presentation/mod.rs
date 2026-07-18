use std::{fmt::Debug, rc::Rc};

use crate::{
    Application,
    data::projector::{BindError, ConnectError, ProjectorTransportTrait as _},
    nav::{NavHost, NavRoute},
    presentation::{
        connecting::ConnectingDestination, connection_failed::ConnectionFailedDestination,
        difficulity_select::DifficultySelectDestination, fallback::FallbackDestination,
        name_input::NameInputDestination, playing::PlayingDestination, ranking::RankingDestination,
        score::ScoreDestination, start::StartScreenDestination,
    },
};
use tokio::sync::oneshot;
use tracing::{debug, info};

/// Binds viewmodel's callback to slint adopter.
macro_rules! bind_callback {
    ($adopter:ident, $viewmodel:ident, $name:ident) => {
        pastey::paste! {
            $adopter.[<on_ $name>]({
                let viewmodel = std::rc::Rc::clone(&$viewmodel);
                move || {
                    viewmodel.[<on_ $name>]();
                }
            })
        }
    };
}

/// Part of [`state_struct`]. This macro requires [`slint::Global`] to exist in scope.
macro_rules! property {
    ($adopter:ident, $name: ident) => {
        pastey::paste! {

            crate::presentation::PropertyHandle {
                getter: std::rc::Rc::new({
                    let adopter_weak = $adopter.as_weak();
                    move || adopter_weak.unwrap().[<get_ $name>]()
                }),
                setter: std::rc::Rc::new({
                    let adopter_weak = $adopter.as_weak();
                    move |$name| adopter_weak.unwrap().[<set_ $name>]($name)
                })
            }
        }
    };
}

/// Creates state struct which contains slint properties.
/// See also: [`property`]
///
/// # Example
/// ```
/// state_struct!(
///    NameInputState,
///    keyboard_mode => KeyBoardMode,
///    player_name_text => SharedString,
///    error_msg => SharedString
/// );
/// ```
///
/// This is equivalent to:
///
/// ```
/// use crate::{presentation::PropertyHandle, ui::KeyboardMode};
/// use slint::SharedString;
///
/// #[allow(dead_code)]
/// struct NameInputState {
///     keyboard_mode: PropertyHandle<KeyBoardMode>,
///     player_name_text: PropertyHandle<SharedString>,
///     error_msg: PropertyHandle<SharedString>,
/// }
///
/// impl NameInputState {
///     fn new(adopter: &ui::NameInputAdopter) -> Self {
///         Self {
///             keyboard_mode: property!(adopter,keyboard_mode),
///             player_name_text: property!(adopter,player_name_text),
///             error_msg: property!(adopter,error_msg),
///         }
///     }
/// }
/// ```
macro_rules! state_struct {
    ($module_name:ident, $($name: ident => $typ: ty),*) => {
        #[allow(unused_imports)]// Used in property! macro.
        use slint::Global as _;

        pastey::paste!{
            #[allow(dead_code)] // propertyを網羅したいため
            #[derive(Debug)]
            struct [<$module_name:camel State>] {
                $(
                    $name: crate::presentation::PropertyHandle<$typ>,
                )*
            }

            impl [<$module_name:camel State>] {
                #[allow(dead_code)] // いずれ全部使う
                fn new(adopter: &ui::[<$module_name:camel Adopter>]) -> Self {
                    Self {
                        $(
                            $name: property!(adopter, $name),
                        )*
                    }
                }
            }
        }
    }
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

/// Wraps slint's property.
struct PropertyWrapper<T> {
    getter: Rc<dyn Fn() -> T>,
    setter: Rc<dyn Fn(T)>,
}

impl<T> PropertyWrapper<T> {
    fn get(&self) -> T {
        (self.getter)()
    }

    fn set(&self, val: T) {
        (self.setter)(val)
    }
}

impl<T> Debug for PropertyWrapper<T>
impl<T> Debug for PropertyHandle<T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PropertyHandle")
            .field("value", &(self.getter)())
            .finish()
    }
}

impl<T> Clone for PropertyWrapper<T> {
    /// Cheap clone (same as [Rc::clone()][std::rc::Rc])
    fn clone(&self) -> Self {
        Self {
            getter: Rc::clone(&self.getter),
            setter: Rc::clone(&self.setter),
        }
    }
}

pub fn init_connection(application: &Application) {
    debug!("initializing connection");

    let transport = application.repositories.projector.clone();
    let transport_clone = application.repositories.projector.clone();
    let (start_tx, start_rx) = oneshot::channel();

    let nav_controller = application.nav_controller.clone();
    let nav_controller2 = nav_controller.clone();

    slint::spawn_local(async move {
        let guard = transport.borrow_mut();
        guard.bind({
            let nav_controller = nav_controller.clone();
            move |res| match res {
                Ok(()) => {
                    info!("successfully bound port for TCP");
                    start_tx.send(Ok(())).unwrap();
                }
                Err(BindError::AlreadyBound) => panic!("this should be first attempt to bind port"),
                Err(BindError::Other(e)) => {
                    start_tx.send(Err(())).unwrap();
                    nav_controller
                        .navigate(NavRoute::Fallback(format!("failed to bind port: {}", e)));
                    return;
                }
            }
        });
    })
    .unwrap();
    slint::spawn_local(async move {
        let guard = transport_clone.borrow_mut();
        if start_rx.await.unwrap().is_err() {
            return;
        };
        info!("connecting to projector");
        guard.connect({
            move |res| match res {
                Ok(()) => {
                    info!("connection succeeds");
                    nav_controller2.navigate(NavRoute::Start);
                }
                Err(ConnectError::PortNotBound) => panic!("bound just before"),
                Err(ConnectError::Tcp(e)) => {
                    nav_controller2.navigate(NavRoute::ConnectionFailed(format!(
                        "failed to accept connection: {}",
                        e
                    )));
                }
                Err(ConnectError::Timeout(_)) => {
                    nav_controller2.navigate(NavRoute::ConnectionFailed(
                        "タイムアウトしました".to_string(),
                    ));
                }
            }
        });
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
mod tests {
    use super::*;
    use tokio::sync::oneshot;

    #[test]
    fn slint_spawn_local_is_reentrant() {
        slint::spawn_local(async move {
            let (tx, rx) = oneshot::channel();

            slint::spawn_local(async move {
                tx.send("Hello!").unwrap();
            })
            .unwrap();
            let msg = rx.await.unwrap();
            assert_eq!("Hello!", msg);
            slint::quit_event_loop();
        })
        .unwrap();

        slint::run_event_loop().unwrap();
    }
}
