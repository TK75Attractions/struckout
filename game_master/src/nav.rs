//! Heavily Inspired by Jetpack Navigation 3 (see https://developer.android.com/guide/navigation).
//!
//! Key types of this module are:
//! - [`NavHost`]
//! - [`NavController`]
//! - [`NavDestination`]

use std::{cell::RefCell, fmt::Debug, rc::Rc};

use crate::ui;

/// Watches [`NavController`] and prepares `ViewModel`s before navigating to the route.
pub struct NavHost {
    destinations: Vec<Box<dyn NavDestination>>,
}

impl NavHost {
    pub fn new() -> Self {
        Self {
            destinations: Vec::new(),
        }
    }

    /// Registers [`NavDestination`] to the host.
    ///
    /// Corresponds to `NavHost`'s block in Jetpack Navigation 3, for instance:
    /// ```kotlin
    /// NavHost(navController = navController, startDestination = Profile) {
    ///     composable<Profile> { ProfileScreen( /* ... */ ) }
    ///     composable<FriendsList> { FriendsListScreen( /* ... */ ) }
    /// }
    /// ```
    pub fn register(&mut self, dest: impl NavDestination + 'static) {
        self.destinations.push(Box::new(dest));
    }

    pub fn on_navigate(&mut self, route: &NavRoute) {
        let dest: Vec<_> = self
            .destinations
            .iter()
            .filter(|d| d.matches(route))
            .collect();
        assert_eq!(dest.len(), 1);
        let dest = dest[0];
        dest.load(route);
    }
}

pub trait NavDestination {
    /// Load viewmodel's state with given arguments by `route`.
    ///
    /// Invariant: given `route`'s variant is the variant which returned `true` in [NavDestination::matches()].
    fn load(&self, route: &NavRoute);

    fn matches(&self, route: &NavRoute) -> bool;
}

/// Clonable handle for navigating routes.
#[derive(Debug)]
pub struct NavController(Rc<RefCell<NavControllerInner>>);

impl NavController {
    pub fn new<F>(default_route: ui::NavRoute, route_property_setter: F) -> Self
    where
        F: Fn(ui::NavRoute) + 'static,
    {
        Self(Rc::new(RefCell::new(NavControllerInner::new(
            default_route,
            route_property_setter,
        ))))
    }

    pub fn navigate(&self, route: NavRoute) {
        self.0.borrow_mut().navigate(route);
    }

    pub fn subscribe_on_navigate<F>(&self, f: F)
    where
        F: FnMut(&NavRoute) + 'static,
    {
        self.0.borrow_mut().subscribe_on_navigate(f);
    }
}

impl Clone for NavController {
    /// Cheap clone (same as [`Rc::clone()`][std::rc::Rc])
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}

/// Navigation routes. Unlike [`ui::NavRoute`], each route can have arguments
/// like [Jetpack's Navigation3](https://developer.android.com/guide/navigation?hl=ja).
#[allow(dead_code)] // いずれ全部カバーする
pub enum NavRoute {
    Start,
    NameInput,
    DifficulitySelect,
    Playing,
    Score,
    Ranking,
    Fallback(String),
}

impl Into<ui::NavRoute> for NavRoute {
    fn into(self) -> ui::NavRoute {
        match self {
            Self::Start => ui::NavRoute::Start,
            Self::NameInput => ui::NavRoute::NameInput,
            Self::DifficulitySelect => ui::NavRoute::DifficulitySelect,
            Self::Playing => ui::NavRoute::Playing,
            Self::Score => ui::NavRoute::Score,
            Self::Ranking => ui::NavRoute::DifficulitySelect,
            Self::Fallback(_) => ui::NavRoute::Fallback,
        }
    }
}

struct NavControllerInner {
    route_property_setter: Box<dyn Fn(ui::NavRoute)>,
    on_navigate: Option<Box<dyn FnMut(&NavRoute)>>,
}

impl Debug for NavControllerInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NavControllerInner")
            .field(
                "on_navigate",
                &if self.on_navigate.is_some() {
                    "Some( .. )"
                } else {
                    "None"
                },
            )
            .finish()
    }
}

impl NavControllerInner {
    fn new<F>(default_route: ui::NavRoute, route_property_setter: F) -> Self
    where
        F: Fn(ui::NavRoute) + 'static,
    {
        route_property_setter(default_route);
        Self {
            on_navigate: None,
            route_property_setter: Box::new(route_property_setter),
        }
    }

    fn navigate(&mut self, route: NavRoute) {
        if let Some(on_navigate) = &mut self.on_navigate {
            (on_navigate.as_mut())(&route);
        }

        let ui_route = route.into();
        (self.route_property_setter)(ui_route);
    }

    fn subscribe_on_navigate<F>(&mut self, f: F)
    where
        F: FnMut(&NavRoute) + 'static,
    {
        self.on_navigate = Some(Box::new(f))
    }
}
