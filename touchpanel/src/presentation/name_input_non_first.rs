use std::fmt::Debug;

use slint::{SharedString, ToSharedString};
use stern::{
    WorkerThread,
    nav::NavDestination,
    worker::{ForegroundExecutor, SlintExecutor},
};
use struckout_proto::types::PlayerId;
use tokio::sync::oneshot;
use touchpanel_ui::{
    KeyBoardMode, NameInputNonFirstPropertyMappers, NameInputNonFirstStates,
    NameInputNonFirstViewModelTrait, NavRoute, NavRouteKind,
};
use tracing::instrument;

use super::pop_player_name;
use crate::{Application, Context, NavController, data::RequestError};

touchpanel_ui::define_name_input_non_first_mapper! {}
viewmodel_rc!(NameInputNonFirstViewModel<Context,SlintExecutor>, NameInputNonFirstAdopter);

#[derive(derive_more::Debug)]
struct NameInputNonFirstViewModel<C, E> {
    state: NameInputNonFirstStates<Mapper>,
    nav_controller: NavController,
    #[debug(skip)]
    worker: WorkerThread<C, E>,
}

impl NameInputNonFirstViewModel<Context, SlintExecutor> {
    fn new(application: &Application) -> Self {
        use slint::{ComponentHandle, Global};
        Self {
            state: NameInputNonFirstStates::<Mapper>::new(
                application
                    .ui
                    .global::<touchpanel_ui::NameInputNonFirstAdopter>()
                    .as_weak(),
            ),
            nav_controller: application.nav_controller.clone(),
            worker: application.worker.clone(),
        }
    }
}

trait GetPlayer {
    fn get_player(
        &self,
        name: impl Into<String> + Send,
    ) -> impl Future<Output = Result<Option<PlayerId>, RequestError>> + Send;
}

impl GetPlayer for Context {
    async fn get_player(
        &self,
        name: impl Into<String> + Send,
    ) -> Result<Option<PlayerId>, RequestError> {
        let mut gm = self.game_master.get().unwrap().clone();
        gm.get_player(name).await
    }
}

impl<C, E> NameInputNonFirstViewModelTrait for NameInputNonFirstViewModel<C, E>
where
    C: Send + Sync + GetPlayer + 'static,
    E: ForegroundExecutor,
{
    #[instrument]
    fn on_push_character(&mut self, char: SharedString) {
        assert_eq!(char.chars().count(), 1, "character length must 1");
        let old_text = self.state.player_name_text.get_ref().clone();
        let new_text = old_text + &char;
        self.state.player_name_text.set(new_text.clone());
    }

    #[instrument]
    fn on_remove_character(&mut self) {
        let old_text = self.state.player_name_text.get_ref().clone();
        if old_text.is_empty() {
            return;
        }

        let new_text = pop_player_name(old_text);
        self.state.player_name_text.set(new_text.clone());
    }

    fn on_submit_name(&mut self) {
        let name = self.state.player_name_text.get_ref().clone();
        let (tx, rx) = oneshot::channel();
        self.worker.spawn_cx(async move |cx| {
            let res = cx.get_player(name).await;
            tx.send(res).unwrap();
        });
        self.worker.spawn_local({
            let nc = self.nav_controller.clone();
            let error_msg_prop = self.state.error_msg.clone();
            async move {
                let res = rx.await.unwrap();
                match res {
                    Ok(Some(player_id)) => {
                        nc.navigate(NavRoute::DifficulitySelect { player_id });
                    }
                    Ok(None) => {
                        error_msg_prop.set("入力されているプレイヤー名は存在しません。以前遊んだ際のプレイヤー名を忘れた場合は一つ前の画面に戻って新規プレイしてください。".to_shared_string());
                    }
                    Err(e) => {
                        error_msg_prop.set(e.to_shared_string());
                    }
                }
            }
        });
    }

    #[instrument]
    fn on_switch_keyboard_mode(&mut self) {
        let old_mode = self.state.keyboard_mode.get();
        let new_mode = match old_mode {
            KeyBoardMode::Hiragana => KeyBoardMode::Katakana,
            KeyBoardMode::Katakana => KeyBoardMode::Hiragana,
        };
        self.state.keyboard_mode.set(new_mode);
    }

    #[instrument]
    fn on_back(&mut self) {
        self.nav_controller.navigate(NavRoute::Start);
    }
}

pub struct NameInputNonFirstDestination(NameInputNonFirstViewModelRc);

impl NameInputNonFirstDestination {
    pub fn new(application: &Application) -> Self {
        Self(NameInputNonFirstViewModelRc::new(application))
    }
}

impl NavDestination<NavRoute> for NameInputNonFirstDestination {
    fn load(&mut self, route: &NavRoute) {
        let NavRoute::NameInputNonFirst = route else {
            panic!("given NavRoute has invalid variant value");
        };
    }

    fn route(&self) -> NavRouteKind {
        NavRouteKind::NameInputNonFirst
    }
}

#[cfg(test)]
mod tests {}
