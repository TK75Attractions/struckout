use std::{sync::Arc, time::Duration};

use crate::{
    Application, Config, Context, NavController,
    data::GameMasterClient,
    presentation::{
        connecting::ConnectingDestination, connection_failed::ConnectionFailedDestination,
        difficulty_select::DifficultySelectDestination, fallback::FallbackDestination,
        name_input_first::NameInputFirstDestination,
        name_input_non_first::NameInputNonFirstDestination, playing::PlayingDestination,
        ranking::RankingDestination, score::ScoreDestination, start::StartScreenDestination,
    },
};
use slint::{SharedString, ToSharedString as _};
use stern::{WorkerThread, nav::NavHost, worker::ForegroundExecutor};
use tokio::{sync::oneshot, time::timeout};
use touchpanel_ui::NavRoute;
use tracing::{debug, warn};

/// Defines `XxxViewModelRc` which wraps `XxxViewModel`.
///
/// `XxxViewModelRc::new()` registers viewmodel to the adopter
/// by calling [`stern::GlobalExt::register_viewmodel()`]).
///
/// `vm` is the name of viewmodel type (e.g. XxxViewModel).
macro_rules! viewmodel_rc {
    ($vm:ident, $adopter:ty) => {
        pastey::paste! {
            #[allow(unused_imports)]
            use slint::ComponentHandle as _;
            #[allow(unused_imports)]
            use stern::GlobalExt as _;

            #[derive(derive_more::Deref)]
            struct [<$vm Rc>](std::rc::Rc<std::cell::RefCell<$vm>>);

            impl [<$vm Rc>] {
                #[doc = concat!("Creates [`", stringify!($vm), "`] and registers it to the adapter by calling [`stern::GlobalExt::register_viewmodel()`].")]
                fn new(application: &Application) -> Self {
                    let this = std::rc::Rc::new(std::cell::RefCell::new($vm::new(application)));
                    application.ui.global::<touchpanel_ui::$adopter>()
                        .register_viewmodel(std::rc::Rc::clone(&this));

                    Self(this)
                }
            }
        }
    };
    ($vm:ident<$generics:ident>, $adopter:ty) => {
        pastey::paste! {
            #[allow(unused_imports)]
            use slint::ComponentHandle as _;
            #[allow(unused_imports)]
            use stern::GlobalExt as _;

            #[derive(derive_more::Deref)]
            struct [<$vm Rc>]<$generics>(std::rc::Rc<std::cell::RefCell<$vm<$generics>>>);

            impl [<$vm Rc>]<crate::Context> {
                #[doc = concat!("Creates [`", stringify!($vm), "`] and registers it to the adapter by calling [`stern::GlobalExt::register_viewmodel()`].")]
                fn new(application: &Application) -> Self {
                    let this = std::rc::Rc::new(std::cell::RefCell::new($vm::new(application)));
                    application.ui.global::<touchpanel_ui::$adopter>()
                        .register_viewmodel(std::rc::Rc::clone(&this));

                    Self(this)
                }
            }
        }
    };
}

pub mod connecting;
pub mod connection_failed;
pub mod difficulty_select;
pub mod fallback;
pub mod name_input_first;
pub mod name_input_non_first;
pub mod playing;
pub mod ranking;
pub mod score;
pub mod start;
//@xtask-pub-mod

/// Connects to game-master's gRPC server. **Must be called within tokio context**.
///
/// When succeeded, it navigates to `StartScreen` and returns connected [`GameMasterClient`].
/// When failed, it navigates to `ConnectionFailedScreen` with error message.
async fn connect_to_game_master<E>(
    worker: &WorkerThread<Context, E>,
    nc: NavController,
    config: Arc<Config>,
) -> Result<GameMasterClient, ()>
where
    E: ForegroundExecutor,
{
    let (tx, rx) = oneshot::channel();

    worker.spawn_cx({
        async move |cx| {
            let res = timeout(
                Duration::from_secs(5),
                GameMasterClient::connect(config.server_addr.as_ref(), config.machine_id.into()),
            )
            .await;
            tx.send(res).unwrap();
        }
    });
    match rx.await.expect("channel should not be closed") {
        Ok(Ok(v)) => {
            nc.navigate(NavRoute::Start);
            Ok(v)
        }
        Ok(Err(e)) => {
            nc.navigate(NavRoute::ConnectionFailed(format!(
                "game-masterへの接続に失敗しました: {}",
                e
            )));
            Err(())
        }
        Err(_) => {
            nc.navigate(NavRoute::ConnectionFailed(
                "タイムアウトしました".to_string(),
            ));
            Err(())
        }
    }
}

/// Initializes [`WorkerThread`]'s context. Note that since the function calls [`slint::spawn_local()`] to await async functions,
/// the process actually starts after slint's event loop has started. (e.g. by [`AppWindow::run()`])
///
/// [AppWindow::run]: crate::ui::AppWindow::run
pub fn init_worker_context(application: &Application) {
    let nc = application.nav_controller.clone();
    let mut worker = application.worker.clone();
    let config = application.config.clone();
    slint::spawn_local(async move {
        debug!("initializing worker context");
        let game_master = match connect_to_game_master(&worker, nc, config).await {
            Ok(v) => v,
            Err(_) => {
                warn!("failed to connect to game-master. user can retry it.");
                return;
            }
        };

        {
            let cx = worker.context();
            cx.game_master
                .set(game_master)
                .expect("this should be a first successful attempt to connect to game-master");
        }
        debug!("worker context initialized successfully");
    })
    .unwrap();
}

/// Registers each [`NavDestination`][crate::nav::NavDestination]s at [`NavHost`].
pub fn attach_navhost(application: &Application) {
    NavHost::builder(application.nav_controller.clone())
        .register(StartScreenDestination::new(&application))
        .register(NameInputFirstDestination::new(&application))
        .register(DifficultySelectDestination::new(&application))
        .register(FallbackDestination::new(&application))
        .register(ConnectionFailedDestination::new(&application))
        .register(PlayingDestination::new(&application))
        .register(ScoreDestination::new(&application))
        .register(ConnectingDestination::new(&application))
        .register(RankingDestination::new(&application))
        .register(NameInputNonFirstDestination::new(&application))
        //@xtask-register
        .finish()
        .expect("failed to build NavHost");
}

/// 最後の文字を消した値を返す
fn pop_player_name(old_text: impl Into<String>) -> SharedString {
    let mut text = old_text.into();
    let _last = text.pop();
    text.to_shared_string()
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
