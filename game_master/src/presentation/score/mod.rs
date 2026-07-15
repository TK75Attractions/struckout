use std::{cell::RefCell, rc::Rc};

use crate::{
    Application,
    nav::{NavController, NavDestination, NavRoute, NavRouteKind},
    session::SessionManager,
    ui,
};
use slint::{ComponentHandle, Global};
use tracing::debug;

state_struct!(
    Score,
    rank => ui::Rank,
    score => i32
);

#[derive(Debug)]
struct ScoreViewModel {
    nav_controller: NavController,
    state: ScoreState,
}

impl ScoreViewModel {
    fn new(nav_controller: NavController, state: ScoreState) -> Self {
        Self {
            nav_controller,
            state,
        }
    }

    fn on_next_clicked(&self) {
        self.nav_controller.navigate(NavRoute::Ranking);
    }
}

pub struct ScoreDestination {
    adopter: slint::Weak<ui::ScoreAdopter<'static>>,
    nav_controller: NavController,
    session_manager: Rc<RefCell<SessionManager>>,
}

impl ScoreDestination {
    pub fn new<PT>(application: &Application<PT>) -> Self {
        Self {
            adopter: application.ui.global::<ui::ScoreAdopter>().as_weak(),
            nav_controller: application.nav_controller.clone(),
            session_manager: application.session_manager.clone(),
        }
    }
}

impl NavDestination for ScoreDestination {
    fn load(&self, route: &NavRoute) {
        debug!("loading ScoreViewModel");
        let NavRoute::Score = route else {
            panic!("matched variant should be given");
        };

        let adopter = self.adopter.unwrap();
        let viewmodel = Rc::new(ScoreViewModel::new(
            self.nav_controller.clone(),
            ScoreState::new(&adopter),
        ));

        let session = self
            .session_manager
            .borrow()
            .session()
            .expect("session should exist when ScoreDestination is loaded");
        viewmodel.state.rank.set(ui::Rank::S); //TODO: 難易度に応じて切り替える
        viewmodel
            .state
            .score
            .set(session.cur_score().try_into().unwrap());

        bind_callback!(adopter, viewmodel, next_clicked);
    }

    fn route(&self) -> NavRouteKind {
        NavRouteKind::Score
    }
}
