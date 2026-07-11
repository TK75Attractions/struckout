use std::rc::Rc;

use slint::{ComponentHandle, Global, SharedString, ToSharedString};
use tracing::{debug, error, trace};

use crate::{
    Application,
    nav::NavController,
    player_repository::{InsertPlayerError, PlayerRepository},
    ui::{self, KeyBoardMode, NavRoute},
};

pub struct NameInputViewModel {
    player_repo: Rc<PlayerRepository>,
    nav_controller: NavController,
    state: NameInputState,
}

#[allow(dead_code)] // propertyを網羅したいため
struct NameInputState {
    keyboard_mode_getter: Rc<dyn Fn() -> KeyBoardMode>,
    keyboard_mode_setter: Rc<dyn Fn(KeyBoardMode)>,
    player_name_text_getter: Rc<dyn Fn() -> SharedString>,
    player_name_text_setter: Rc<dyn Fn(SharedString)>,
    error_msg_getter: Rc<dyn Fn() -> SharedString>,
    error_msg_setter: Rc<dyn Fn(SharedString)>,
}

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

        let old_mode = (self.state.keyboard_mode_getter)();
        let new_mode = match old_mode {
            KeyBoardMode::Hiragana => KeyBoardMode::Katakana,
            KeyBoardMode::Katakana => KeyBoardMode::Hiragana,
        };
        (self.state.keyboard_mode_setter)(new_mode);
    }

    fn on_push_character(&self, char: SharedString) {
        trace!("NameInputViewModel::on_push_character");

        assert_eq!(char.chars().count(), 1, "character length must 1");
        let old_text = (self.state.player_name_text_getter)();
        let new_text = old_text + &char;
        (self.state.player_name_text_setter)(new_text);
    }

    fn on_remove_character(&self) {
        trace!("NameInputViewModel::on_remove_character");

        let old_text = (self.state.player_name_text_getter)();
        if old_text.is_empty() {
            return;
        }

        let new_text = pop_player_name(old_text);
        (self.state.player_name_text_setter)(new_text)
    }

    fn on_submit_name(&self) {
        trace!("NameInputViewModel::on_submit_name");

        let name = (self.state.player_name_text_getter)();
        let set_msg = Rc::clone(&self.state.error_msg_setter);
        let nav_controller = self.nav_controller.clone();
        self.player_repo.insert_player(name, move |res| match res {
            Ok(()) => {
                nav_controller.navigate(NavRoute::DifficulitySelect);
            }
            Err(InsertPlayerError::NameAlredyInUse(name)) => {
                set_msg(format!("'{}'はすでに使われています", name).to_shared_string());
            }
            Err(InsertPlayerError::Sqlx(e)) => {
                set_msg("プログラム内部でエラーが発生しました".to_shared_string());
                error!(?e, "error occured while inserting player");
            }
        });
    }
}

/// 最後の文字を消した値を返す
fn pop_player_name(old_text: SharedString) -> SharedString {
    old_text[0..old_text.len() - 1].to_shared_string()
}

pub fn init(application: &Application) {
    debug!("initializing NameInputViewModel");

    let adopter = application.ui.global::<ui::NameInputAdopter>();
    let state = NameInputState {
        keyboard_mode_getter: Rc::new({
            let adopter_weak = adopter.as_weak();
            move || adopter_weak.unwrap().get_keyboard_mode()
        }),
        keyboard_mode_setter: Rc::new({
            let adopter_weak = adopter.as_weak();
            move |mode| adopter_weak.unwrap().set_keyboard_mode(mode)
        }),
        player_name_text_getter: Rc::new({
            let adopter_weak = adopter.as_weak();
            move || adopter_weak.unwrap().get_player_name_text()
        }),
        player_name_text_setter: Rc::new({
            let adopter_weak = adopter.as_weak();
            move |val| adopter_weak.unwrap().set_player_name_text(val)
        }),
        error_msg_getter: Rc::new({
            let adopter_weak = adopter.as_weak();
            move || adopter_weak.unwrap().get_error_msg()
        }),
        error_msg_setter: Rc::new({
            let adopter_weak = adopter.as_weak();
            move |val| adopter_weak.unwrap().set_error_msg(val)
        }),
    };
    let viewmodel = Rc::new(NameInputViewModel::new(
        Rc::clone(&application.repositories.player),
        application.nav_controller.clone(),
        state,
    ));

    adopter.on_switch_keyboard_mode({
        let viewmodel = Rc::clone(&viewmodel);
        move || {
            viewmodel.on_switch_keyboard_mode();
        }
    });
    adopter.on_push_character({
        let viewmodel = Rc::clone(&viewmodel);
        move |char| {
            viewmodel.on_push_character(char);
        }
    });
    adopter.on_remove_character({
        let viewmodel = Rc::clone(&viewmodel);
        move || {
            viewmodel.on_remove_character();
        }
    });
    adopter.on_submit_name({
        let viewmodel = Rc::clone(&viewmodel);
        move || {
            viewmodel.on_submit_name();
        }
    });
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
