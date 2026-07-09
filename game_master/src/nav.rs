use std::{cell::RefCell, rc::Rc};

use crate::ui;

pub struct NavController(Rc<RefCell<NavControllerInner<Box<dyn Fn(ui::NavRoute)>>>>);

impl NavController {
    pub fn new<F>(default_route: ui::NavRoute, route_property_setter: F) -> Self
    where
        F: Fn(ui::NavRoute) + 'static,
    {
        Self(Rc::new(RefCell::new(NavControllerInner::new(
            default_route,
            Box::new(route_property_setter),
        ))))
    }

    pub fn navigate(&self, route: ui::NavRoute) {
        self.0.borrow_mut().navigate(route);
    }
}

impl Clone for NavController {
    /// Cheap clone (same as [`Rc::clone()`][std::rc::Rc])
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}

struct NavControllerInner<F> {
    route_property_setter: F,
}

impl<F> NavControllerInner<F>
where
    F: Fn(ui::NavRoute),
{
    fn new(default_route: ui::NavRoute, route_property_setter: F) -> Self {
        route_property_setter(default_route);
        Self {
            route_property_setter,
        }
    }

    fn navigate(&mut self, route: ui::NavRoute) {
        (self.route_property_setter)(route);
    }
}
