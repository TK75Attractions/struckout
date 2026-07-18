use std::rc::Rc;

use slint::{ComponentHandle, Global, SharedString, ToSharedString};
use tracing::{debug, error, trace};

use crate::{
    Application,
    data::player::{InsertPlayerError, PlayerRepository},
    nav::{NavController, NavDestination, NavRoute, NavRouteKind},
    ui::{self, KeyBoardMode},
};

#[derive(Debug)]
pub struct NameInputViewModel {
    player_repo: Rc<PlayerRepository>,
    nav_controller: NavController,
    state: NameInputState,
}

state_struct!(
    NameInput,
    keyboard_mode => KeyBoardMode,
    player_name_text => SharedString,
    error_msg => SharedString
);

impl NameInputViewModel {
    fn new(
        player_repo: Rc<PlayerRepository>,
        nav_controller: NavController,
        state: NameInputState,
    ) -> Self {
        Self {
            player_repo,
            nav_controller,
            state,
        }
    }

    fn on_switch_keyboard_mode(&self) {
        trace!("NameInputViewModel::on_switch_keyboard_mode");

        let old_mode = self.state.keyboard_mode.get();
        let new_mode = match old_mode {
            KeyBoardMode::Hiragana => KeyBoardMode::Katakana,
            KeyBoardMode::Katakana => KeyBoardMode::Hiragana,
        };
        self.state.keyboard_mode.set(new_mode);
    }

    fn on_push_character(&self, char: SharedString) {
        trace!("NameInputViewModel::on_push_character");

        assert_eq!(char.chars().count(), 1, "character length must 1");
        let old_text = self.state.player_name_text.get();
        let new_text = old_text + &char;
        self.state.player_name_text.set(new_text);
    }

    fn on_remove_character(&self) {
        trace!("NameInputViewModel::on_remove_character");

        let old_text = self.state.player_name_text.get();
        if old_text.is_empty() {
            return;
        }

        let new_text = pop_player_name(old_text);
        self.state.player_name_text.set(new_text)
    }

    fn on_submit_name(&self) {
        trace!("NameInputViewModel::on_submit_name");

        let name = self.state.player_name_text.get();
        let msg = self.state.error_msg.clone();
        let nav_controller = self.nav_controller.clone();
        self.player_repo.insert_player(name, move |res| match res {
            Ok(()) => {
                nav_controller.navigate(NavRoute::DifficulitySelect);
            }
            Err(InsertPlayerError::NameAlredyInUse(name)) => {
                msg.set(format!("'{}'はすでに使われています", name).to_shared_string());
            }
            Err(InsertPlayerError::Sqlx(e)) => {
                msg.set("プログラム内部でエラーが発生しました".to_shared_string());
                error!(?e, "error occured while inserting player");
            }
        });
    }
}

/// 最後の文字を消した値を返す
fn pop_player_name(old_text: SharedString) -> SharedString {
    old_text[0..old_text.len() - 1].to_shared_string()
}

pub struct NameInputDestination {
    nav_controller: NavController,
    adopter: slint::Weak<ui::NameInputAdopter<'static>>,
    player_repo: Rc<PlayerRepository>,
}

impl NameInputDestination {
    pub fn new(application: &Application) -> Self {
        Self {
            nav_controller: application.nav_controller.clone(),
            adopter: application.ui.global::<ui::NameInputAdopter>().as_weak(),
            player_repo: application.repositories.player.clone(),
        }
    }
}

impl NavDestination for NameInputDestination {
    fn load(&self, route: &NavRoute) {
        debug!("loading NameInputViewModel");
        let NavRoute::NameInput = route else {
            panic!("matched variant should be given");
        };

        let adopter = self.adopter.unwrap();

        let viewmodel = Rc::new(NameInputViewModel::new(
            Rc::clone(&self.player_repo),
            self.nav_controller.clone(),
            NameInputState::new(&adopter),
        ));

        macro_rules! cb {
            ($name: ident) => {
                bind_callback!(adopter, viewmodel, $name);
            };
        }
        adopter.on_push_character({
            let viewmodel = Rc::clone(&viewmodel);
            move |char| {
                viewmodel.on_push_character(char);
            }
        });
        cb!(switch_keyboard_mode);
        cb!(remove_character);
        cb!(submit_name);
    }

    fn route(&self) -> NavRouteKind {
        NavRouteKind::NameInput
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pop_player_name_works() {
        let old_text = "bobb".to_shared_string();
        let new_text = pop_player_name(old_text);
        assert_eq!("bob", new_text.as_str());
    }
}
