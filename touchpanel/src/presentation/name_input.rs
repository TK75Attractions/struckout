use slint::{ComponentHandle, Global, SharedString, ToSharedString};
use stern::{
    WorkerThread,
    nav::NavDestination,
    worker::{ForegroundExecutor, SlintExecutor},
};
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

// viewmodel_rc!(NameInputViewModel<C>, NameInputAdopter);

// TODO: 一タイプごとにvalidateする

#[derive(Debug)]
pub struct NameInputViewModel<C, E> {
    pub nav_controller: NavController,
    pub worker: WorkerThread<C, E>,
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

impl NameInputViewModel<Context, SlintExecutor> {
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
impl<C, E> NameInputViewModel<C, E>
where
    C: PlayerRepository,
    E: ForegroundExecutor,
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

impl<C, E> NameInputViewModelTrait for NameInputViewModel<C, E>
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

/// 最後の文字を消した値を返す
fn pop_player_name(old_text: impl Into<String>) -> SharedString {
    let mut text = old_text.into();
    let _last = text.pop();
    text.to_shared_string()
}

pub struct NameInputDestination(
    #[allow(dead_code)] // may used when some arg is added to the route
    NameInputViewModel<Context, SlintExecutor>,
);

impl NameInputDestination {
    pub fn new(application: &Application) -> Self {
        Self(NameInputViewModel::new(application))
    }
}

impl NavDestination<NavRoute> for NameInputDestination {
    fn load(&mut self, route: &NavRoute) {
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
    use crate::{
        data::RequestError,
        presentation::name_input::{NameInputViewModel, PlayerRepository},
    };
    use rstest::rstest;
    use slint::ToSharedString;

    use std::{
        cell::RefCell,
        pin::Pin,
        rc::Rc,
        sync::{
            Arc,
            atomic::{AtomicU8, Ordering},
        },
        time::Duration,
    };
    use stern::{WorkerThread, nav::NavController, worker::SmolExecutor};
    use struckout_proto::{
        types::PlayerId,
        validate_player_name_response::{self, ValidatePlayerNameResp},
    };
    use tokio::sync::oneshot;
    use tonic::Status;
    use touchpanel_ui::{NameInputStates, NavRoute, UiNavRoute};
    use tracing::trace;

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

    #[rstest]
    #[case::cancelled(Duration::from_millis(5000), "")]
    #[case::not_cancelled(Duration::from_millis(1000), "プレイヤー名は既に使われています")]
    fn push_character_validation_test(
        #[case] first_call_dur: Duration,
        #[case] error_msg: &'static str,
    ) {
        let sub = tracing_subscriber::FmtSubscriber::builder()
            .with_max_level(tracing::Level::TRACE)
            .with_test_writer()
            .finish();
        // Subscriber is already set by another test case,
        tracing::subscriber::set_global_default(sub).ok();

        i_slint_backend_testing::init_integration_test_with_system_time();

        slint::invoke_from_event_loop(|| {
            println!("Hello!");
        })
        .unwrap();
        slint::spawn_local(async {}).expect("event loop should exist");

        let tracker = Tracker::new();
        let mut test =
            NameInputScreenTest::new(FakePlayerRepository::new().on_validate_player_name({
                let tracker = tracker.clone();
                move |_| {
                    let tracker = tracker.clone();
                    Box::pin(async move {
                        tracker.called();
                        trace!(?tracker, "called tracker");
                        match tracker.count() {
                            1 => {
                                tokio::time::sleep(first_call_dur).await;
                                Ok(ValidatePlayerNameResp::AlreadyUsed(
                                    validate_player_name_response::AlreadyUsed {},
                                ))
                            }
                            2 => Ok(ValidatePlayerNameResp::Ok(
                                validate_player_name_response::Ok {},
                            )),
                            _ => unimplemented!(),
                        }
                    })
                }
            }));

        let join = test.vm.on_push_character_impl("テ".to_shared_string());

        let (tx, rx) = oneshot::channel();
        test.worker.spawn_cx(async move |_| {
            tokio::time::sleep(Duration::from_millis(4000)).await;
            tx.send(()).unwrap();
        });
        slint::spawn_local(async move {
            rx.await.unwrap();
            trace!("calling next push");
            test.vm.on_push_character_impl("ス".to_shared_string());
            assert_eq!(test.vm.state.error_msg.get_ref().as_str(), error_msg);
            trace!(?tracker, "tracker status");
            // i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(200));
        })
        .unwrap();

        slint::spawn_local(async move {
            join.await;
            slint::quit_event_loop().unwrap();
            test.worker.shutdown();
        })
        .unwrap();

        println!("running event loop...");
        slint::run_event_loop().unwrap();
    }

    #[derive(derive_more::Debug)]
    pub struct FakePlayerRepository {
        #[debug(skip)]
        on_validate_player_name: Box<
            dyn Fn(
                    String,
                ) -> Pin<
                    Box<dyn Future<Output = Result<ValidatePlayerNameResp, RequestError>> + Send>,
                > + Send
                + Sync,
        >,
        #[debug(skip)]
        on_add_player: Box<
            dyn Fn(String) -> Pin<Box<dyn Future<Output = Result<PlayerId, Status>> + Send>>
                + Send
                + Sync,
        >,
    }

    impl FakePlayerRepository {
        pub fn new() -> Self {
            Self {
                on_validate_player_name: Box::new(|_| Box::pin(async { unimplemented!() })),
                on_add_player: Box::new(|_| Box::pin(async { unimplemented!() })),
            }
        }

        pub fn on_validate_player_name(
            mut self,
            f: impl Fn(
                String,
            ) -> Pin<
                Box<dyn Future<Output = Result<ValidatePlayerNameResp, RequestError>> + Send>,
            > + Send
            + Sync
            + 'static,
        ) -> Self {
            self.on_validate_player_name = Box::new(f);
            self
        }

        pub fn on_add_player(
            mut self,
            f: impl Fn(String) -> Pin<Box<dyn Future<Output = Result<PlayerId, Status>> + Send>>
            + Send
            + Sync
            + 'static,
        ) -> Self {
            self.on_add_player = Box::new(f);
            self
        }
    }

    impl PlayerRepository for FakePlayerRepository {
        async fn validate_player_name(
            &self,
            name: impl Into<String> + Send,
        ) -> Result<ValidatePlayerNameResp, RequestError> {
            (self.on_validate_player_name)(name.into()).await
        }

        async fn add_player(&self, name: impl Into<String> + Send) -> Result<PlayerId, Status> {
            (self.on_add_player)(name.into()).await
        }
    }

    struct NameInputScreenTest {
        pub route: Rc<RefCell<UiNavRoute>>,
        pub worker: WorkerThread<FakePlayerRepository, SmolExecutor>,
        pub vm: NameInputViewModel<FakePlayerRepository, SmolExecutor>,
    }

    impl NameInputScreenTest {
        pub fn new(cx: FakePlayerRepository) -> Self {
            let route = Rc::new(RefCell::new(UiNavRoute::Start));

            let nav_controller = NavController::new(NavRoute::Start, {
                let route = Rc::clone(&route);
                move |new| {
                    let mut guard = route.borrow_mut();
                    *guard = new.into();
                }
            });
            let worker = WorkerThread::new_smol(cx);
            let (state, _mock) = NameInputStates::new_mocked();
            let vm = NameInputViewModel {
                nav_controller,
                worker: worker.clone(),
                state,
                gm_cancel_tok: None,
            };
            Self { route, worker, vm }
        }
    }

    #[derive(Debug, Clone)]
    struct Tracker {
        count: Arc<AtomicU8>,
    }

    impl Tracker {
        fn new() -> Self {
            Self {
                count: Arc::new(AtomicU8::new(0)),
            }
        }

        /// Increments count.
        fn called(&self) {
            self.count.fetch_add(1, Ordering::SeqCst);
        }

        fn count(&self) -> u8 {
            self.count.load(Ordering::Relaxed)
        }
    }
}
