//! Implementation for [GameMasterPlayerService].

use struckout_proto::{
    AddPlayerRequest, AddPlayerResponse, GetPlayerRequest, GetPlayerResponse,
    ValidatePlayerNameRequest, ValidatePlayerNameResponse,
    game_master_player_service_server::GameMasterPlayerService,
    validate_player_name_response::ValidatePlayerNameResp,
};
use tonic::{Request, Response, Status};

use crate::{
    AddPlayerError, DataSource, DataSourceImpl, GetPlayerError, ValidatePlayerNameError,
    data::Player,
};

pub struct GameMasterPlayerServiceImpl<D = DataSourceImpl> {
    data_source: D,
}

impl<D> GameMasterPlayerServiceImpl<D> {
    /// Constructs new [`GameMasterPlayerServiceImpl`] with the given data source.
    pub fn new(data_source: D) -> Self {
        Self { data_source }
    }
}

#[tonic::async_trait]
impl<D> GameMasterPlayerService for GameMasterPlayerServiceImpl<D>
where
    D: DataSource,
{
    async fn add_player(
        &self,
        req: Request<AddPlayerRequest>,
    ) -> Result<Response<AddPlayerResponse>, Status> {
        let req = req.into_inner();
        match self.data_source.add_player(req.name).await {
            Ok(player_id) => Ok(Response::new(AddPlayerResponse {
                player_id: player_id.into_inner(),
            })),
            Err(e @ AddPlayerError::NameAlreadyUsed) => Err(Status::already_exists(e.to_string())),
            Err(AddPlayerError::Sqlx(e)) => Err(Status::unavailable(e.to_string())),
        }
    }

    async fn validate_player_name(
        &self,
        req: Request<ValidatePlayerNameRequest>,
    ) -> Result<Response<ValidatePlayerNameResponse>, Status> {
        let req = req.into_inner();
        match self.data_source.validate_player_name(req.player_name).await {
            Ok(_) => Ok(Response::new(ValidatePlayerNameResponse {
                validate_player_name_resp: Some(ValidatePlayerNameResp::Ok(
                    struckout_proto::validate_player_name_response::Ok {},
                )),
            })),
            Err(ValidatePlayerNameError::AlreadyUsed(_name)) => {
                Ok(Response::new(ValidatePlayerNameResponse {
                    validate_player_name_resp: Some(ValidatePlayerNameResp::AlreadyUsed(
                        struckout_proto::validate_player_name_response::AlreadyUsed {},
                    )),
                }))
            }
            Err(ValidatePlayerNameError::Sqlx(e)) => Err(Status::internal(e.to_string())),
        }
    }

    async fn get_player(
        &self,
        req: Request<GetPlayerRequest>,
    ) -> Result<Response<GetPlayerResponse>, Status> {
        let req = req.into_inner();
        match self.data_source.get_player(req.name).await {
            Ok(Player { player_id, name }) => Ok(Response::new(GetPlayerResponse {
                player_id: player_id.into_inner(),
                player_name: name,
            })),
            Err(GetPlayerError::NotExist(name)) => Err(Status::not_found(format!(
                "player with name '{}' does not exist",
                name
            ))),
            Err(GetPlayerError::Sqlx(e)) => Err(Status::internal(e.to_string())),
        }
    }
}
