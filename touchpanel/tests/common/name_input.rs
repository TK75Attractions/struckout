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

use slint::ToSharedString;
use stern::{WorkerThread, nav::NavController};
use struckout_proto::{
    types::PlayerId,
    validate_player_name_response::{self, ValidatePlayerNameResp},
};
use tokio::sync::oneshot;
use tonic::Status;
use touchpanel::{
    data::RequestError,
    presentation::name_input::{NameInputViewModel, PlayerRepository},
};
use touchpanel_ui::{NameInputStates, NavRoute, UiNavRoute};
use tracing::trace;

#[derive(derive_more::Debug)]
pub struct FakePlayerRepository {
    #[debug(skip)]
    on_validate_player_name: Box<
        dyn Fn(
                String,
            )
                -> Pin<Box<dyn Future<Output = Result<ValidatePlayerNameResp, RequestError>> + Send>>
            + Send
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
        )
            -> Pin<Box<dyn Future<Output = Result<ValidatePlayerNameResp, RequestError>> + Send>>
        + Send
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

pub struct NameInputScreenTest {
    pub route: Rc<RefCell<UiNavRoute>>,
    pub worker: WorkerThread<FakePlayerRepository>,
    pub vm: NameInputViewModel<FakePlayerRepository>,
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
        let worker = WorkerThread::new(cx);
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
pub struct Tracker {
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

pub fn push_character_validation_test(first_call_dur: Duration, error_msg: &'static str) {
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
    let mut test = NameInputScreenTest::new(FakePlayerRepository::new().on_validate_player_name({
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
