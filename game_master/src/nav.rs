//! Heavily Inspired by Jetpack Navigation 3 (see https://developer.android.com/guide/navigation).
//!
//! Key types of this module are:
//! - [`NavHost`]
//! - [`NavController`]
//! - [`NavDestination`]

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    fmt::Debug,
    rc::Rc,
};
use strum::{EnumDiscriminants, EnumIter, IntoEnumIterator};
use thiserror::Error;
use tracing::debug;

use crate::ui;

/// Watches [`NavController`] and prepares `ViewModel`s before navigating to the route.
///
/// Typically you would attach [`NavHost`] to [`NavController`] using [`NavHost::builder()`].
pub struct NavHost {
    destinations: HashMap<NavRouteKind, Box<dyn NavDestination>>,
}

impl NavHost {
    fn on_navigate(&mut self, route: &NavRoute) {
        debug!(?route, "NavHost: searching registered destination");
        let dest = self.destinations.get(&route.into()).unwrap(); // checked in Builder
        dest.load(route);
    }

    pub fn builder(nav_controller: NavController) -> NavHostBuilder {
        NavHostBuilder {
            nav_controller,
            destinations: Vec::new(),
        }
    }
}

/// Builder for [`NavHost`]. You can cerate new builder from [`NavHost::builder()`].
///
/// Created [`NavHost`] will be moved to [`NavController`] by calling [`NavController::subscribe_on_navigate()`].
pub struct NavHostBuilder {
    nav_controller: NavController,
    destinations: Vec<Box<dyn NavDestination>>,
}

impl NavHostBuilder {
    /// Registers [`NavDestination`] to the host.
    ///
    /// Corresponds to `NavHost`'s block in Jetpack Navigation 3, for instance:
    /// ```kotlin
    /// NavHost(navController = navController, startDestination = Profile) {
    ///     composable<Profile> { ProfileScreen( /* ... */ ) }
    ///     composable<FriendsList> { FriendsListScreen( /* ... */ ) }
    /// }
    /// ```
    pub fn register(mut self, dest: impl NavDestination + 'static) -> Self {
        self.destinations.push(Box::new(dest));
        self
    }

    pub fn finish(self) -> Result<(), NavHostBuilderError> {
        // TODO: 重複とかの確認
        let mut map = HashMap::new();
        let mut duplicates = HashSet::new();
        for dest in self.destinations {
            if let Some(old) = map.insert(dest.route(), dest) {
                duplicates.insert(old.route());
            }
        }
        if !duplicates.is_empty() {
            return Err(NavHostBuilderError::DuplicatedRegisteration(duplicates));
        }

        let mut missings = HashSet::new();
        NavRouteKind::iter().for_each(|route| {
            if !map.contains_key(&route) {
                missings.insert(route);
            }
        });
        if !missings.is_empty() {
            return Err(NavHostBuilderError::NotAllRoutesRegistered(missings));
        }

        let mut host = NavHost { destinations: map };
        self.nav_controller.subscribe_on_navigate(move |route| {
            host.on_navigate(route);
        });
        Ok(())
    }
}

/// Error returned from [`NavHostBuilder::finish()`].
#[derive(Debug, Error)]
pub enum NavHostBuilderError {
    #[error("destinations for some routes are not registered: {0:?}")]
    NotAllRoutesRegistered(HashSet<NavRouteKind>),
    #[error("destination for {0:?} are registered multiple times")]
    DuplicatedRegisteration(HashSet<NavRouteKind>),
}

pub trait NavDestination {
    /// Load viewmodel's state with given arguments by `route`.
    ///
    /// Invariant: given `route`'s variant is the variant which returned `true` in [NavDestination::matches()].
    fn load(&self, route: &NavRoute);

    fn route(&self) -> NavRouteKind;
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

    fn subscribe_on_navigate<F>(&self, f: F)
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
#[derive(Debug, Clone, EnumDiscriminants)]
#[strum_discriminants(name(NavRouteKind), derive(EnumIter, Hash))]
pub enum NavRoute {
    Start,
    NameInput,
    DifficulitySelect,
    Playing(ui::Difficulity),
    Score,
    Ranking,
    Fallback(String),
    ConnectionFailed(String),
    Connecting,
}

impl Into<ui::NavRoute> for NavRoute {
    fn into(self) -> ui::NavRoute {
        match self {
            Self::Start => ui::NavRoute::Start,
            Self::NameInput => ui::NavRoute::NameInput,
            Self::DifficulitySelect => ui::NavRoute::DifficulitySelect,
            Self::Playing(_) => ui::NavRoute::Playing,
            Self::Score => ui::NavRoute::Score,
            Self::Ranking => ui::NavRoute::DifficulitySelect,
            Self::Fallback(_) => ui::NavRoute::Fallback,
            Self::ConnectionFailed(_) => ui::NavRoute::ConnectionFailed,
            Self::Connecting => ui::NavRoute::Connecting,
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
        debug!(?route, "navigating");
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

#[cfg(test)]
mod tests {
    use crate::presentation::start::StartScreenDestination;

    use super::*;

    #[test]
    fn navhost_builder_returns_err_when_not_all_routes_registered() {
        todo!()
        /*let nav_controller = NavController::new(ui::NavRoute::Start, |route| {
            println!("dummy");
        });
        let application =
        NavHost::builder(nav_controller).register(StartScreenDestination::new(application));*/
    }

    #[test]
    fn navhost_builder_returns_err_when_registerations_are_duplicated() {
        todo!()
    }
}
