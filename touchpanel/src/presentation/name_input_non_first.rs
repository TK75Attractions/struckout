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
    KeyBoardMode, MultiPlayAvailability, NameInputNonFirstPropertyMappers, NameInputNonFirstStates,
    NameInputNonFirstViewModelTrait, NavRoute, NavRouteKind,
};
use tracing::instrument;

use crate::{
    Application, Context, NavController,
    data::RequestError,
    keyboard::{keyboard_modify, pop_player_name},
    presentation::{SingleSessionStarter, route_after_player},
};

touchpanel_ui::define_name_input_non_first_mapper! {}
viewmodel_rc!(NameInputNonFirstViewModel<Context,SlintExecutor>, NameInputNonFirstAdopter);

#[derive(derive_more::Debug)]
struct NameInputNonFirstViewModel<C, E> {
    state: NameInputNonFirstStates<Mapper>,
    nav_controller: NavController,
    #[debug(skip)]
    worker: WorkerThread<C, E>,
    multiplay_availability: MultiPlayAvailability,
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
            multiplay_availability: application.config.multiplay_availability,
        }
    }
}

impl<C, E> NameInputNonFirstViewModel<C, E> {
    fn reset(&self) {
        self.state.player_name_text.set("".into());
        self.state.keyboard_mode.set(KeyBoardMode::Hiragana);
        self.state.error_msg.set("".into());
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
    C: GetPlayer + SingleSessionStarter,
    E: ForegroundExecutor,
{
    #[instrument]
    fn on_push_character(&mut self, char: SharedString) {
        let cnt = char.chars().count();
        if cnt == 0 {
            return;
        } else if cnt != 1 {
            unreachable!("character length must be 1");
        }
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
        let multiplay_availability = self.multiplay_availability;
        let (tx, rx) = oneshot::channel();
        self.worker.spawn_cx(async move |cx| {
            let res = cx.get_player(name).await;
            let res = match res {
                Ok(Some(player_id)) => {
                    route_after_player(cx.as_ref(), multiplay_availability, player_id)
                        .await
                        .map(Some)
                        .map_err(|error| error.to_string())
                }
                Ok(None) => Ok(None),
                Err(error) => Err(error.to_string()),
            };
            tx.send(res).unwrap();
        });
        self.worker.spawn_local({
            let nc = self.nav_controller.clone();
            let error_msg_prop = self.state.error_msg.clone();
            async move {
                let res = rx.await.unwrap();
                match res {
                    Ok(Some(route)) => {
                        nc.navigate(route);
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
    fn on_set_keyboard_mode(&mut self, mode: KeyBoardMode) {
        self.state.keyboard_mode.set(mode);
    }

    #[instrument]
    fn on_modify(&mut self) {
        let name = self.state.player_name_text.get_ref().clone();
        let modified = keyboard_modify(name);
        self.state.player_name_text.set(modified.to_shared_string());
    }
}

pub struct NameInputNonFirstDestination(
    #[allow(unused)] // just for viewmodel initialization
    NameInputNonFirstViewModelRc,
);

impl NameInputNonFirstDestination {
    pub fn new(application: &Application) -> Self {
        Self(NameInputNonFirstViewModelRc::new(application))
    }
}

impl NavDestination<NavRoute> for NameInputNonFirstDestination {
    fn load(&mut self, route: &NavRoute) {
        let NavRoute::NameInputNonFirst = route else {
            unreachable!();
        };
        self.0.borrow().reset();
    }

    fn route(&self) -> NavRouteKind {
        NavRouteKind::NameInputNonFirst
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use stern::{WorkerThread, nav::NavController, worker::SmolExecutor};
    use touchpanel_ui::UiNavRoute;

    use super::*;

    #[test]
    fn reset_clears_previous_input() {
        let route = Rc::new(RefCell::new(UiNavRoute::Start));
        let nav_controller = NavController::new(NavRoute::Start, {
            let route = Rc::clone(&route);
            move |new| *route.borrow_mut() = new
        });
        let (state, _mock) = NameInputNonFirstStates::new_mocked();
        let vm = NameInputNonFirstViewModel {
            state,
            nav_controller,
            worker: WorkerThread::<Context, SmolExecutor>::new_smol(Context::new_empty()),
            multiplay_availability: MultiPlayAvailability::Enabled,
        };
        vm.state.player_name_text.set("previous".into());
        vm.state.keyboard_mode.set(KeyBoardMode::Number);
        vm.state.error_msg.set("error".into());

        vm.reset();

        assert_eq!(vm.state.player_name_text.get_ref().as_str(), "");
        assert_eq!(vm.state.keyboard_mode.get(), KeyBoardMode::Hiragana);
        assert_eq!(vm.state.error_msg.get_ref().as_str(), "");
    }
}
