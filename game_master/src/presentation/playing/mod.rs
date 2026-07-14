use crate::{
    Application,
    nav::{NavController, NavDestination, NavRoute},
    session::{RemainingTime, SessionManager, SessionSubscriber},
    ui,
};
use slint::{ComponentHandle, Global, SharedString, ToSharedString};
use std::{cell::RefCell, rc::Rc};
use tracing::debug;

state_struct!(Playing,
    score => i32,
    remaining_time => SharedString
);

struct PlayingViewModel {
    nav_controller: NavController,
    state: PlayingState,
}

impl PlayingViewModel {
    fn on_session_ends(&self) {
        self.nav_controller.navigate(NavRoute::Score);
    }
}

impl SessionSubscriber for PlayingViewModel {
    fn on_score_changed(&self, score: u32) {
        self.state.score.set(score.try_into().unwrap());
    }

    fn on_remaining_time_changed(&self, remaining_time: &RemainingTime) {
        self.state
            .remaining_time
            .set(format!("{}", remaining_time).to_shared_string());
    }
}

pub struct PlayingDestination {
    adopter: slint::Weak<ui::PlayingAdopter<'static>>,
    nav_controller: NavController,
    session_manager: Rc<RefCell<SessionManager>>,
}

impl PlayingDestination {
    pub fn new<PT>(application: &Application<PT>) -> Self {
        Self {
            adopter: application.ui.global::<ui::PlayingAdopter>().as_weak(),
            nav_controller: application.nav_controller.clone(),
            session_manager: application.session_manager.clone(),
        }
    }
}

impl NavDestination for PlayingDestination {
    fn load(&self, route: &NavRoute) {
        debug!("loading PlayingViewModel");

        let NavRoute::Playing = route else {
            panic!("matched variant should be given");
        };

        let adopter = self.adopter.unwrap();
        let viewmodel = Rc::new(PlayingViewModel {
            nav_controller: self.nav_controller.clone(),
            state: PlayingState::new(&adopter),
        });

        let mut session_manager = self.session_manager.borrow_mut();
        session_manager.subscribe(Rc::clone(&viewmodel));
        session_manager.start_session({
            let viewmodel = viewmodel.clone();
            move || {
                viewmodel.on_session_ends();
            }
        });
    }

    fn matches(&self, route: &NavRoute) -> bool {
        matches!(route, &NavRoute::Playing)
    }
}
