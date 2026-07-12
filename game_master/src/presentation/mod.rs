use std::{fmt::Debug, rc::Rc};

use crate::{
    Application,
    data::projector::{BindError, ProjectorConnection},
    nav::{NavHost, NavRoute},
    presentation::{
        difficulity_select::DifficultySelectDestination, fallback::FallbackDestination,
        name_input::NameInputDestination, start::StartScreenDestination,
    },
};

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

            crate::presentation::PropertyWrapper {
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
/// use crate::{presentation::PropertyWrapper, ui::KeyboardMode};
/// use slint::SharedString;
///
/// #[allow(dead_code)]
/// struct NameInputState {
///     keyboard_mode: PropertyWrapper<KeyBoardMode>,
///     player_name_text: PropertyWrapper<SharedString>,
///     error_msg: PropertyWrapper<SharedString>,
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
                    $name: crate::presentation::PropertyWrapper<$typ>,
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
where
    T: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PropertyWrapper")
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

fn init_connection<PT>(application: &Application<PT>)
where
    PT: ProjectorConnection,
{
    let transport = application.repositories.projector.borrow_mut();
    let nav_controller = application.nav_controller.clone();
    transport.bind(move |res| match res {
        Ok(()) => {}

        Err(BindError::AlreadyBound) => panic!("this should be first attempt to bind port"),
        Err(BindError::Other(e)) => {
            nav_controller.navigate(NavRoute::Fallback(format!("failed to bind port: {}", e)));
        }
    });
}

/// Registers each [`NavDestination`][crate::nav::NavDestination]s at [`NavHost`].
pub fn register_destinations<PT>(nav_host: &mut NavHost, application: &Application<PT>)
where
    PT: ProjectorConnection + 'static,
{
    nav_host.register(StartScreenDestination::new(&application));
    nav_host.register(NameInputDestination::new(&application));
    nav_host.register(DifficultySelectDestination::new(&application));
    nav_host.register(FallbackDestination::new(&application));
}
