use slint::{ComponentHandle, Global, SharedString, ToSharedString};
use stern::{
    PropertyHandle, WorkerThread,
    nav::NavDestination,
    worker::{ForegroundExecutor, SlintExecutor},
};
use struckout_proto::{types::PlayerId, validate_player_name_response::ValidatePlayerNameResp};
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use tonic::Status;
use tracing::{Instrument, debug, instrument, trace};

use super::pop_player_name;
use crate::{Application, Context, NavController, data::RequestError};

use touchpanel_ui::{
    KeyBoardMode, NameInputFirstPropertyMappers, NameInputFirstStates,
    NameInputFirstViewModelTrait, NavRoute, NavRouteKind,
};

touchpanel_ui::define_name_input_first_mapper! {}

viewmodel_rc!(NameInputViewModel<Context, SlintExecutor>, NameInputFirstAdopter);

#[derive(derive_more::Debug)]
pub struct NameInputViewModel<C, E> {
    pub nav_controller: NavController,
    #[debug(skip)]
    pub worker: WorkerThread<C, E>,
    pub state: NameInputFirstStates<Mapper>,
    /// Token to cancel request to game-master.
    pub gm_cancel_tok: Option<CancellationToken>,
    show_topbar_prop: PropertyHandle<bool>,
    show_backbutton_prop: PropertyHandle<bool>,
    #[debug(skip)]
    callback_setter: Box<dyn Fn(Box<dyn FnMut()>)>,
}

#[cfg_attr(test, mockall::automock)]
pub trait PlayerRepository: Send + Sync + 'static {
    fn validate_player_name<T: Into<String> + Send + 'static>(
        &self,
        name: T,
    ) -> impl Future<Output = Result<ValidatePlayerNameResp, RequestError>> + Send;

    fn add_player<T: Into<String> + Send + 'static>(
        &self,
        name: T,
    ) -> impl Future<Output = Result<PlayerId, Status>> + Send;
}

impl PlayerRepository for Context {
    async fn validate_player_name<T: Into<String> + Send + 'static>(
        &self,
        name: T,
    ) -> Result<ValidatePlayerNameResp, RequestError> {
        let mut gm = self.game_master.get().unwrap().clone();
        gm.validate_player_name(name).await
    }

    async fn add_player<T: Into<String> + Send + 'static>(
        &self,
        name: T,
    ) -> Result<PlayerId, Status> {
        let mut gm = self.game_master.get().unwrap().clone();
        gm.add_player(name).await
    }
}

impl NameInputViewModel<Context, SlintExecutor> {
    fn new(application: &Application) -> Self {
        Self {
            nav_controller: application.nav_controller.clone(),
            worker: application.worker.clone(),
            state: NameInputFirstStates::<Mapper>::new(
                application
                    .ui
                    .global::<touchpanel_ui::NameInputFirstAdopter>()
                    .as_weak(),
            ),
            gm_cancel_tok: None,
            show_topbar_prop: application.show_topbar_prop(),
            show_backbutton_prop: application.show_backbutton_prop(),
            callback_setter: Box::new(application.click_backbutton_callback_setter()),
        }
    }
}

impl<C, E> NameInputViewModel<C, E>
where
    C: PlayerRepository,
    E: ForegroundExecutor,
{
    /// Validates `name`. Returns the handle to the task which handles validation result.
    fn validate_new_name(&mut self, name: impl Into<String>) -> E::SpawnedHandle {
        let name = name.into();

        self.cancel_previous_request();

        let cancel_tok = CancellationToken::new();
        self.gm_cancel_tok = Some(cancel_tok.clone());
        let (tx, rx) = oneshot::channel();
        self.worker.spawn_spanned(async move |cx| {
            trace!(name, "validating");
            tokio::select! {
                _ = cancel_tok.cancelled() => {
                    tx.send(None).unwrap();
                },
                res = cx.validate_player_name(name) => {
                    tx.send(Some(res)).unwrap();
                }
            }
        });
        self.worker.spawn_local({
            let nc = self.nav_controller.clone();
            let msg_prop = self.state.error_msg.clone();
            async move {
                match rx.await.unwrap() {
                    Some(Ok(ValidatePlayerNameResp::Ok(_))) => {
                        msg_prop.set("".to_shared_string());
                    }
                    Some(Ok(ValidatePlayerNameResp::AlreadyUsed(_))) => {
                        msg_prop.set("プレイヤー名は既に使われています".to_shared_string());
                    }
                    Some(Err(e)) => {
                        nc.navigate(NavRoute::Fallback(e.to_string()));
                    }
                    // cancelled
                    None => trace!("validation request cancelled"),
                }
            }
            .in_current_span()
        })
    }

    /// Cancels previous request to game-master.
    fn cancel_previous_request(&mut self) {
        if let Some(tok) = &self.gm_cancel_tok {
            tok.cancel();
        }
    }

    #[instrument(skip(self), level = "debug")]
    pub fn on_push_character_impl(&mut self, char: SharedString) -> E::SpawnedHandle {
        trace!("NameInputViewModel::on_push_character");

        assert_eq!(char.chars().count(), 1, "character length must 1");
        let old_text = self.state.player_name_text.get_ref().clone();
        let new_text = old_text + &char;
        self.state.player_name_text.set(new_text.clone());

        self.validate_new_name(new_text)
    }
}

impl<C, E> NameInputFirstViewModelTrait for NameInputViewModel<C, E>
where
    C: PlayerRepository,
    E: ForegroundExecutor,
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

pub struct NameInputFirstDestination(
    #[allow(dead_code)] // may used when some arg is added to the route
    NameInputViewModelRc,
);

impl NameInputFirstDestination {
    pub fn new(application: &Application) -> Self {
        Self(NameInputViewModelRc::new(application))
    }
}

impl NavDestination<NavRoute> for NameInputFirstDestination {
    fn load(&mut self, route: &NavRoute) {
        debug!("loading NameInputViewModel");
        let NavRoute::NameInputFirst = route else {
            panic!("matched variant should be given");
        };

        let vm = self.0.borrow();
        vm.show_topbar_prop.set(true);
        vm.show_backbutton_prop.set(true);

        let nc = vm.nav_controller.clone();
        (vm.callback_setter)(Box::new(move || nc.navigate(NavRoute::Start)));
    }

    fn route(&self) -> NavRouteKind {
        NavRouteKind::NameInputFirst
    }
}

#[cfg(test)]
mod tests {
    use std::{assert_matches, cell::RefCell, ops::ControlFlow, rc::Rc, sync::Arc};

    use i_slint_backend_testing::ElementRoot;
    use rstest::rstest;
    use stern::worker::SmolExecutor;
    use struckout_proto::validate_player_name_response;
    use time::ext::NumericalDuration;
    use touchpanel_ui::UiNavRoute;

    use crate::{Config, presentation::attach_navhost};

    use super::*;

    #[rstest]
    #[case::cancelled(300.milliseconds(), 200.milliseconds(), 400.milliseconds(), "dummy!", "")]
    #[case::not_cancelled(
        300.milliseconds(),
        400.milliseconds(),
        200.milliseconds(),
        "プレイヤー名は既に使われています",
        "",
    )]
    fn push_character_validation_test(
        // validationにかかる時間
        #[case] first_call_dur: time::SignedDuration,
        // 一回目の呼び出しと二回目の呼び出しの間 (i.e. ユーザーの入力間隔)
        #[case] dur_between_two_calls: time::SignedDuration,
        // validationにかかる時間
        #[case] second_call_dur: time::SignedDuration,
        #[case] error_msg_after_first_call_dur: &'static str,
        #[case] error_msg_after_between_plus_second: &'static str,
    ) {
        // Arrange
        assert!(first_call_dur.is_positive());
        assert!(second_call_dur.is_positive());
        assert!(dur_between_two_calls.is_positive());

        let mut pr = MockPlayerRepository::new();
        let mut pr_mock_seq = mockall::Sequence::new();
        pr.expect_validate_player_name()
            .times(1)
            .in_sequence(&mut pr_mock_seq)
            .return_once(move |_: String| {
                Box::pin(async move {
                    tokio::time::sleep(first_call_dur.unsigned_abs()).await;
                    Ok(ValidatePlayerNameResp::AlreadyUsed(
                        validate_player_name_response::AlreadyUsed {},
                    ))
                })
            });
        pr.expect_validate_player_name()
            .times(1)
            .in_sequence(&mut pr_mock_seq)
            .return_once(move |_: String| {
                Box::pin(async move {
                    tokio::time::sleep(second_call_dur.unsigned_abs()).await;
                    Ok(ValidatePlayerNameResp::Ok(
                        validate_player_name_response::Ok {},
                    ))
                })
            });
        let test = NameInputScreenTest::new(pr);
        test.vm.state.error_msg.set("dummy!".to_shared_string());

        // Act
        let worker = test.worker.clone();
        worker.spawn_local({
            let NameInputScreenTest { worker, mut vm, .. } = test;
            async move {
                // first call
                let first_handle = vm.on_push_character_impl("テ".to_shared_string());
                let error_msg_prop = vm.state.error_msg.clone();
                worker.spawn_local({
                    let worker = worker.clone();
                    async move {
                        worker
                            .foreground_executor()
                            .after(dur_between_two_calls.unsigned_abs())
                            .await;
                        // second call
                        let second_handle = vm.on_push_character_impl("ス".to_shared_string());
                        second_handle.await;
                        assert_eq!(
                            vm.state.error_msg.get_ref().as_str(),
                            error_msg_after_between_plus_second,
                            "error_msg_after_between_plus_second"
                        );
                    }
                });

                // add 10ms as margin to process validation result
                worker
                    .foreground_executor()
                    .after((first_call_dur + 10.milliseconds()).unsigned_abs())
                    .await;
                assert_eq!(
                    error_msg_prop.get_ref().as_str(),
                    error_msg_after_first_call_dur,
                    "error_msg_after_first_call_dur"
                );
                worker.shutdown_all();
            }
        });

        worker.foreground_executor().start();
    }

    #[test]
    fn backbutton_is_shown_and_moves_back_to_start_when_clicked() {
        // Arrange
        let ui = touchpanel_ui::AppWindow::new().unwrap();

        let nav_controller = NavController::new(NavRoute::Connecting, {
            let ui = ui.as_weak();

            move |route| {
                let ui = ui.unwrap();
                ui.set_nav_route(route);
            }
        });

        let cx = Context::new_empty();
        let worker = WorkerThread::new(cx);

        let application = Application {
            nav_controller,
            ui,
            worker,
            config: Arc::new(Config {
                server_addr: "127.0.0.1".to_string(),
                machine_id: 1,
            }),
        };
        attach_navhost(&application);

        // Act
        application
            .nav_controller
            .navigate(NavRoute::NameInputFirst);
        let root = application.ui.root_element();

        let btn = root
            .query_descendants()
            .match_accessible_label("root_back-button")
            .find_first()
            .expect("top-bar and back-button should be set visible");
        btn.invoke_accessible_default_action();
        application.nav_controller.with_current(|r| {
            assert_matches!(r, NavRoute::Start);
        });
    }

    struct NameInputScreenTest {
        pub route: Rc<RefCell<UiNavRoute>>,
        pub worker: WorkerThread<MockPlayerRepository, SmolExecutor>,
        pub vm: NameInputViewModel<MockPlayerRepository, SmolExecutor>,
    }

    impl NameInputScreenTest {
        pub fn new(cx: MockPlayerRepository) -> Self {
            let route = Rc::new(RefCell::new(UiNavRoute::Start));
            let show_topbar = Rc::new(RefCell::new(false));
            let show_backbutton = Rc::new(RefCell::new(false));

            let nav_controller = NavController::new(NavRoute::Start, {
                let route = Rc::clone(&route);
                move |new| {
                    let mut guard = route.borrow_mut();
                    *guard = new;
                }
            });
            let worker = WorkerThread::new_smol(cx);
            let (state, _mock) = NameInputFirstStates::new_mocked();
            let vm = NameInputViewModel {
                nav_controller,
                worker: worker.clone(),
                state,
                gm_cancel_tok: None,
                show_topbar_prop: PropertyHandle::new(move |new| {
                    let mut guard = show_topbar.borrow_mut();
                    *guard = new;
                }),
                show_backbutton_prop: PropertyHandle::new(move |new| {
                    let mut guard = show_backbutton.borrow_mut();
                    *guard = new;
                }),
                callback_setter: Box::new(|_| unimplemented!()),
            };
            Self { route, worker, vm }
        }
    }
}
