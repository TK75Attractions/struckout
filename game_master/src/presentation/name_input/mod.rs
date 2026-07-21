use std::rc::Rc;

use slint::{ComponentHandle, Global, SharedString, ToSharedString};
use stern::nav::NavDestination;
use tracing::{debug, error, trace};

use crate::{
    Application, NavController,
    data::player::{InsertPlayerError, PlayerRepository},
    ui::{self, KeyBoardMode, NameInputStates, NameInputViewModelTrait, NavRoute, NavRouteKind},
};

viewmodel_rc!(NameInputViewModel, NameInputAdopter);

#[derive(Debug)]
struct NameInputViewModel {
    player_repo: Rc<PlayerRepository>,
    nav_controller: NavController,
    state: NameInputStates,
}

impl NameInputViewModel {
    fn new(application: &Application) -> Self {
        Self {
            player_repo: application.repositories.player.clone(),
            nav_controller: application.nav_controller.clone(),
            state: NameInputStates::new(application.ui.global::<ui::NameInputAdopter>().as_weak()),
        }
    }
}
impl NameInputViewModelTrait for NameInputViewModel {
    fn on_switch_keyboard_mode(&mut self) {
        trace!("NameInputViewModel::on_switch_keyboard_mode");

        let old_mode = self.state.keyboard_mode.get();
        let new_mode = match old_mode {
            KeyBoardMode::Hiragana => KeyBoardMode::Katakana,
            KeyBoardMode::Katakana => KeyBoardMode::Hiragana,
        };
        self.state.keyboard_mode.set(new_mode);
    }

    fn on_push_character(&mut self, char: SharedString) {
        trace!("NameInputViewModel::on_push_character");

        assert_eq!(char.chars().count(), 1, "character length must 1");
        let old_text = self.state.player_name_text.get();
        let new_text = old_text + &char;
        self.state.player_name_text.set(new_text);
    }

    fn on_remove_character(&mut self) {
        trace!("NameInputViewModel::on_remove_character");

        let old_text = self.state.player_name_text.get();
        if old_text.is_empty() {
            return;
        }

        let new_text = pop_player_name(old_text);
        self.state.player_name_text.set(new_text)
    }

    fn on_submit_name(&mut self) {
        trace!("NameInputViewModel::on_submit_name");

        let name = self.state.player_name_text.get();
        // let msg = self.state.error_msg.clone();
        let nav_controller = self.nav_controller.clone();
        self.player_repo.insert_player(name, move |res| match res {
            Ok(()) => {
                nav_controller.navigate(NavRoute::DifficulitySelect);
            }
            Err(InsertPlayerError::NameAlredyInUse(name)) => {
                todo!();
                // msg.set(format!("'{}'はすでに使われています", name).to_shared_string());
            }
            Err(InsertPlayerError::Sqlx(e)) => {
                // msg.set("プログラム内部でエラーが発生しました".to_shared_string());
                error!(?e, "error occured while inserting player");
                todo!();
            }
        });
    }
}

/// 最後の文字を消した値を返す
fn pop_player_name(old_text: SharedString) -> SharedString {
    old_text[0..old_text.len() - 1].to_shared_string()
}

pub struct NameInputDestination(NameInputViewModelRc);

impl NameInputDestination {
    pub fn new(application: &Application) -> Self {
        Self(NameInputViewModelRc::new(application))
    }
}

impl NavDestination<NavRoute> for NameInputDestination {
    fn load(&self, route: &NavRoute) {
        debug!("loading NameInputViewModel");
        let NavRoute::NameInput = route else {
            panic!("matched variant should be given");
        };

        // do nothing because the route doesn't have any arguments
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
