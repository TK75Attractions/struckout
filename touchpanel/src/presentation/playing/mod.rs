use crate::{
    Application, NavController,
    session::{RemainingTime, SessionManager, SessionSubscriber},
    ui::{self, NavRoute, NavRouteKind, PlayingStates, PlayingViewModelTrait},
};
use slint::{ComponentHandle, Global, ToSharedString};
use stern::nav::NavDestination;
use std::{cell::RefCell, rc::Rc};
use tracing::debug;

viewmodel_rc!(PlayingViewModel, PlayingAdopter);

struct PlayingViewModel {
    nav_controller: NavController,
    state: PlayingStates,
}

impl PlayingViewModel {
    fn new(application: &Application) -> Self {
        Self {
            nav_controller: application.nav_controller.clone(),
            state: PlayingStates::new(application.ui.global::<ui::PlayingAdopter>().as_weak()),
        }
    }

    fn on_session_ends(&self) {
        self.nav_controller.navigate(NavRoute::Score);
    }
}

impl PlayingViewModelTrait for PlayingViewModel {}

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
    session_manager: Rc<RefCell<SessionManager>>,
    viewmodel: PlayingViewModelRc,
}

impl PlayingDestination {
    pub fn new(application: &Application) -> Self {
        Self {
            session_manager: application.session_manager.clone(),
            viewmodel: PlayingViewModelRc::new(application),
        }
    }
}

impl NavDestination<NavRoute> for PlayingDestination {
    fn load(&self, route: &NavRoute) {
        debug!("loading PlayingViewModel");

        let NavRoute::Playing(difficulty) = route else {
            panic!("matched variant should be given");
        };

        /*let mut session_manager = self.session_manager.borrow_mut();
        session_manager.subscribe(Rc::clone(&viewmodel));
        session_manager.start_session(*difficulty, {
            let viewmodel = viewmodel.clone();
            move || {
                viewmodel.on_session_ends();
            }
        });*/
        todo!()
    }

    fn route(&self) -> NavRouteKind {
        NavRouteKind::Playing
    }
}
