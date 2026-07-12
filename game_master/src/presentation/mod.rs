use std::rc::Rc;

use crate::Application;

/// Binds viewmodel's callback to slint adopter.
macro_rules! bind_callback {
    ($adopter:ident, $viewmodel:ident, $name:ident) => {
        pastey::paste! {
            $adopter.[<on_ $name>]({
                let viewmodel = Rc::clone(&$viewmodel);
                move || {
                    viewmodel.[<on_ $name>]();
                }
            })
        }
    };
}

macro_rules! property {
    ($adopter:ident, $name: ident) => {
        pastey::paste! {
            PropertyWrapper {
                getter: Rc::new({
                    let adopter_weak = $adopter.as_weak();
                    move || adopter_weak.unwrap().[<get_ $name>]()
                }),
                setter: Rc::new({
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
    ($struct_name:ident, $($name: ident => $typ: ty),*) => {
        #[allow(dead_code)] // propertyを網羅したいため
        struct $struct_name {
            $(
                $name: PropertyWrapper<$typ>,
            )*
        }

        impl $struct_name {
            fn new(adopter: &ui::NameInputAdopter) -> Self {
                Self {
                    $(
                        $name: property!(adopter, $name),
                    )*
                }
            }
        }
    }
}

pub mod difficulity_select;
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

impl<T> Clone for PropertyWrapper<T> {
    /// Cheap clone (same as [Rc::clone()][std::rc::Rc])
    fn clone(&self) -> Self {
        Self {
            getter: Rc::clone(&self.getter),
            setter: Rc::clone(&self.setter),
        }
    }
}

pub fn init(application: &Application) {
    start::init(application);
    name_input::init(application);
    difficulity_select::init(application);
    playing::init(application);
    score::init(application);
    ranking::init(application);
}
