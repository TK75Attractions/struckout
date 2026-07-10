use std::rc::Rc;

use slint::{ComponentHandle, Global, SharedString, ToSharedString};
use tracing::{debug, trace};

use crate::{
    Application,
    player_repository::PlayerRepository,
    ui::{self, KeyBoardMode},
};

pub struct NameInputViewModel {
    player_repo: Rc<PlayerRepository>,
    state: NameInputState,
}

struct NameInputState {
    keyboard_mode_getter: Box<dyn Fn() -> KeyBoardMode>,
    keyboard_mode_setter: Box<dyn Fn(KeyBoardMode)>,
    player_name_text_getter: Box<dyn Fn() -> SharedString>,
    player_name_text_setter: Box<dyn Fn(SharedString)>,
}

impl NameInputViewModel {
    pub fn new(player_repo: Rc<PlayerRepository>, state: NameInputState) -> Self {
        Self { player_repo, state }
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

        assert_eq!(char.len(), 1, "character length must 1");
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
        self.player_repo.insert_player(name);
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
        keyboard_mode_getter: Box::new({
            let adopter_weak = adopter.as_weak();
            move || adopter_weak.unwrap().get_keyboard_mode()
        }),
        keyboard_mode_setter: Box::new({
            let adopter_weak = adopter.as_weak();
            move |mode| adopter_weak.unwrap().set_keyboard_mode(mode)
        }),
        player_name_text_getter: Box::new({
            let adopter_weak = adopter.as_weak();
            move || adopter_weak.unwrap().get_player_name_text()
        }),
        player_name_text_setter: Box::new({
            let adopter_weak = adopter.as_weak();
            move |val| adopter_weak.unwrap().set_player_name_text(val)
        }),
    };
    let viewmodel = Rc::new(NameInputViewModel::new(
        Rc::clone(&application.repositories.player),
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
