use std::pin::Pin;

use tokio_stream::Stream;
use tonic::{Request, Response};

use crate::proto::{
    AddScoreRequest, AddScoreResponse, Event, ListenEventsRequest, StartGameRequest,
    game_master_service_server::GameMasterService,
};

pub mod proto {
    tonic::include_proto!("tk75attractions.struckout.v1");
}

#[derive(Debug, Clone, Copy)]
pub struct GameId(i32);

#[derive(Debug, Clone, Copy)]
pub struct MachineId(i32);

pub struct Game {
    machine_id: MachineId,
    game_id: GameId,
    score: i32,
}

pub struct GameMasterServiceImpl {
    running_games: Vec<Game>,
}

#[tonic::async_trait]
impl GameMasterService for GameMasterServiceImpl {
    type StartGameStream = Pin<Box<dyn Stream<Item = Result<Event, tonic::Status>> + Send>>;

    type ListenEventsStream = Pin<Box<dyn Stream<Item = Result<Event, tonic::Status>> + Send>>;

    async fn start_game(
        &self,
        req: Request<StartGameRequest>,
    ) -> Result<Response<Self::StartGameStream>, tonic::Status> {
        todo!()
    }

    async fn listen_events(
        &self,
        req: Request<ListenEventsRequest>,
    ) -> Result<Response<Self::ListenEventsStream>, tonic::Status> {
        todo!()
    }

    async fn add_score(
        &self,
        req: Request<AddScoreRequest>,
    ) -> Result<Response<AddScoreResponse>, tonic::Status> {
        todo!()
    }
}
