use crate::{Application, Context, NavController};
use slint::{ComponentHandle, Global};
use stern::{WorkerThread, nav::NavDestination};
use struckout_proto::types::GameId;
use tokio::sync::oneshot;
use touchpanel_ui::{
    NavRoute, NavRouteKind, ScorePropertyMappers, ScoreStates, ScoreViewModelTrait,
};
use tracing::debug;

touchpanel_ui::define_score_mapper! {}

viewmodel_rc!(ScoreViewModel<C>, ScoreAdopter);

#[derive(Debug)]
struct ScoreViewModel<C> {
    worker: WorkerThread<C>,
    nav_controller: NavController,
    state: ScoreStates<Mapper>,
}
impl ScoreViewModel<Context> {
    fn new(application: &Application) -> Self {
        Self {
            worker: application.worker.clone(),
            nav_controller: application.nav_controller.clone(),
            state: ScoreStates::<Mapper>::new(
                application
                    .ui
                    .global::<touchpanel_ui::ScoreAdopter>()
                    .as_weak(),
            ),
        }
    }
}

impl<C> ScoreViewModel<C> {
    fn show_result(&self, game_id: GameId)
    where
        C: GameResultProvider,
    {
        let (tx, rx) = oneshot::channel();

        self.worker.spawn_cx({
            async move |cx| {
                // context is initialized before navigated to StartScreen
                let res = cx.get_game_result(game_id).await;
                tx.send(res).unwrap();
            }
        });

        slint::spawn_local({
            let nc = self.nav_controller.clone();
            let score_prop = self.state.score.clone();

            async move {
                match rx.await.expect("channel should not be closed") {
                    Ok(v) => {
                        score_prop.set(v.try_into().unwrap());
                    }
                    Err(e) => {
                        nc.navigate(NavRoute::Fallback(e.to_string()));
                    }
                };
            }
        })
        .unwrap();
    }
}

impl<C> ScoreViewModelTrait for ScoreViewModel<C> {
    fn on_next_clicked(&mut self) {
        self.nav_controller.navigate(NavRoute::Ranking);
    }
}

trait GameResultProvider: Send + Sync + 'static {
    fn get_game_result(
        &self,
        game_id: GameId,
    ) -> impl Future<Output = Result<u32, tonic::Status>> + Send;
}

impl GameResultProvider for Context {
    async fn get_game_result(&self, game_id: GameId) -> Result<u32, tonic::Status> {
        let mut gm = self.game_master.get().unwrap().clone();
        gm.get_game_result(game_id).await
    }
}

pub struct ScoreDestination {
    worker: WorkerThread<Context>,
    viewmodel: ScoreViewModelRc<Context>,
}

impl ScoreDestination {
    pub fn new(application: &Application) -> Self {
        Self {
            worker: application.worker.clone(),
            viewmodel: ScoreViewModelRc::new(application),
        }
    }
}

impl NavDestination<NavRoute> for ScoreDestination {
    fn load(&self, route: &NavRoute) {
        debug!("loading ScoreViewModel");
        let NavRoute::Score { game_id } = route else {
            panic!("matched variant should be given");
        };

        self.viewmodel.borrow().show_result(*game_id);
    }

    fn route(&self) -> NavRouteKind {
        NavRouteKind::Score
    }
}

#[cfg(test)]
mod tests {}
