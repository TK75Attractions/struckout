use slint::{ComponentHandle, Global, SharedString, ToSharedString};
use stern::{WorkerThread, nav::NavDestination};
use struckout_proto::{types::PlayerId, validate_player_name_response::ValidatePlayerNameResp};
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use tonic::Status;
use tracing::{Instrument, debug, instrument, trace};

use crate::{Application, Context, NavController, data::RequestError};

use touchpanel_ui::{
    KeyBoardMode, NameInputPropertyMappers, NameInputStates, NameInputViewModelTrait, NavRoute,
    NavRouteKind,
};

touchpanel_ui::define_name_input_mapper! {}

viewmodel_rc!(NameInputViewModel<C>, NameInputAdopter);

// TODO: 一タイプごとにvalidateする

#[derive(Debug)]
pub struct NameInputViewModel<C> {
    pub nav_controller: NavController,
    pub worker: WorkerThread<C>,
    pub state: NameInputStates<Mapper>,
    /// Token to cancel request to game-master.
    pub gm_cancel_tok: Option<CancellationToken>,
}

pub trait PlayerRepository: Send + Sync + 'static {
    fn validate_player_name(
        &self,
        name: impl Into<String> + Send,
    ) -> impl Future<Output = Result<ValidatePlayerNameResp, RequestError>> + Send;

    fn add_player(
        &self,
        name: impl Into<String> + Send,
    ) -> impl Future<Output = Result<PlayerId, Status>> + Send;
}

impl PlayerRepository for Context {
    async fn validate_player_name(
        &self,
        name: impl Into<String> + Send,
    ) -> Result<ValidatePlayerNameResp, RequestError> {
        let mut gm = self.game_master.get().unwrap().clone();
        gm.validate_player_name(name).await
    }

    async fn add_player(&self, name: impl Into<String> + Send) -> Result<PlayerId, Status> {
        let mut gm = self.game_master.get().unwrap().clone();
        gm.add_player(name).await
    }
}

impl NameInputViewModel<Context> {
    fn new(application: &Application) -> Self {
        Self {
            nav_controller: application.nav_controller.clone(),
            worker: application.worker.clone(),
            state: NameInputStates::<Mapper>::new(
                application
                    .ui
                    .global::<touchpanel_ui::NameInputAdopter>()
                    .as_weak(),
            ),
            gm_cancel_tok: None,
        }
    }
}
impl<C> NameInputViewModel<C>
where
    C: PlayerRepository,
{
    fn validate_new_name(&mut self, name: impl Into<String>) -> slint::JoinHandle<()> {
        let name = name.into();

        self.cancel_previous_request();

        let cancel_tok = CancellationToken::new();
        self.gm_cancel_tok = Some(cancel_tok.clone());
        let (tx, rx) = oneshot::channel();
        self.worker.spawn_spanned(async move |cx| {
            trace!(name, "validating");
            tokio::select! {
                _ = cancel_tok.cancelled() => {
                    trace!("cancelled");
                    tx.send(None).unwrap();
                },
                res = cx.validate_player_name(name) => {
                    trace!("not cancelled");
                    tx.send(Some(res)).unwrap();
                }
            }
        });
        slint::spawn_local({
            let nc = self.nav_controller.clone();
            let msg_prop = self.state.error_msg.clone();
            async move {
                match rx.await.unwrap() {
                    Some(Ok(ValidatePlayerNameResp::Ok(_))) => (),
                    Some(Ok(ValidatePlayerNameResp::AlreadyUsed(_))) => {
                        msg_prop.set("プレイヤー名は既に使われています".to_shared_string());
                    }
                    Some(Err(e)) => {
                        nc.navigate(NavRoute::Fallback(e.to_string()));
                    }
                    // cancelled
                    None => (),
                }
                trace!("finished");
            }
            .in_current_span()
        })
        .unwrap()
    }

    /// Cancels previous request to game-master.
    fn cancel_previous_request(&mut self) {
        if let Some(tok) = &self.gm_cancel_tok {
            tok.cancel();
        }
    }

    #[instrument(skip(self), level = "debug")]
    pub fn on_push_character_impl(&mut self, char: SharedString) -> slint::JoinHandle<()> {
        trace!("NameInputViewModel::on_push_character");

        assert_eq!(char.chars().count(), 1, "character length must 1");
        let old_text = self.state.player_name_text.get_ref().clone();
        let new_text = old_text + &char;
        self.state.player_name_text.set(new_text.clone());

        self.validate_new_name(new_text)
    }
}

impl<C> NameInputViewModelTrait for NameInputViewModel<C>
where
    C: PlayerRepository,
{
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
        self.on_push_character_impl(char);
    }

    fn on_remove_character(&mut self) {
        trace!("NameInputViewModel::on_remove_character");

        let old_text = self.state.player_name_text.get_ref().clone();
        if old_text.is_empty() {
            return;
        }

        let new_text = pop_player_name(old_text);
        self.state.player_name_text.set(new_text.clone());

        self.validate_new_name(new_text);
    }

    fn on_submit_name(&mut self) {
        trace!("NameInputViewModel::on_submit_name");

        let name = self.state.player_name_text.get_ref().clone();
        let msg = self.state.error_msg.clone();
        let nc = self.nav_controller.clone();
        let (tx, rx) = oneshot::channel();
        self.worker.spawn_cx(async move |cx| {
            let res = cx.add_player(name).await;
            tx.send(res).unwrap();
        });
        slint::spawn_local(async move {
            match rx.await.unwrap() {
                Ok(player_id) => {
                    nc.navigate(NavRoute::DifficulitySelect { player_id });
                }
                Err(e) => {
                    msg.set(
                        format!("プレイヤー名の設定中にエラーが発生しました: {}", e)
                            .to_shared_string(),
                    );
                }
            }
        })
        .unwrap();
    }
}

/// 最後の文字を消した値を返す
fn pop_player_name(old_text: impl Into<String>) -> SharedString {
    let mut text = old_text.into();
    let _last = text.pop();
    text.to_shared_string()
}

pub struct NameInputDestination(
    #[allow(dead_code)] // may used when some arg is added to the route
    NameInputViewModelRc<Context>,
);

impl NameInputDestination {
    pub fn new(application: &Application) -> Self {
        Self(NameInputViewModelRc::new(application))
    }
}

impl NavDestination<NavRoute> for NameInputDestination {
    fn load(&self, route: &NavRoute) {
        debug!("loading NameInputViewModel");
        let NavRoute::NameInput(mode) = route else {
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

    #[test]
    fn pop_player_name_works_with_multi_byte_characters() {
        let old_text = "たろうう".to_shared_string();
        let new_text = pop_player_name(old_text);
        assert_eq!("たろう", new_text.as_str());
    }
}
