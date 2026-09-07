use std::ffi::OsStr;

use game_master::{
    Config, DataSourceImpl, GameMasterServiceImpl,
    proto::game_master_service_server::GameMasterServiceServer,
};
use sqlx::{MySql, Pool, mysql::MySqlPoolOptions};
use thiserror::Error;
use tonic::transport::Server;
use tracing::{Level, error, info};
use tracing_subscriber::FmtSubscriber;

const ENV_MYSQL_ROOT_PASSWORD: &str = "MYSQL_ROOT_PASSWORD";
const ENV_MYSQL_DB_NAME: &str = "MYSQL_DATABASE";

#[tokio::main]
async fn main() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("failed to set default suvscriber");

    let config = match Config::from_file("./config.toml") {
        Ok(v) => v,
        Err(err) => {
            error!(?err, "failed to load config from file");
            std::process::exit(1);
        }
    };

    info!("creating MySQL pool");
    let pool = match new_pool().await {
        Ok(p) => p,
        Err(err) => {
            error!(?err, "failed to create MySQL pool");
            std::process::exit(1);
        }
    };
    info!("succeed to create MySQL pool");

    let data_source = DataSourceImpl::new(pool);
    let game_master = GameMasterServiceImpl::new(data_source);
    let addr = format!("0.0.0.0:{}", config.port)
        .parse()
        .expect("address format should be correct");
    match Server::builder()
        .add_service(GameMasterServiceServer::new(game_master))
        .serve(addr)
        .await
    {
        Ok(_) => (),
        Err(err) => {
            error!(?err, "an error occured");
            std::process::exit(1);
        }
    };
}

#[derive(Debug, Error)]
pub enum PoolCreationError {
    #[error(
        "failed to create MySQL pool: environment variable {name} is missing or invalid: {err}"
    )]
    NoEnvVar {
        name: String,
        err: std::env::VarError,
    },
    #[error(transparent)]
    ConectError(#[from] sqlx::Error),
}

async fn new_pool() -> Result<Pool<MySql>, PoolCreationError> {
    let password = env_var(ENV_MYSQL_ROOT_PASSWORD)?;
    let db_name = env_var(ENV_MYSQL_DB_NAME)?;
    let pool = MySqlPoolOptions::new()
        .connect(format!("mysql://root:{}@db:3306/{}", password, db_name).as_str())
        .await?;
    Ok(pool)
}

/// Reads a environment variable specified by `key`.
///
/// Returns [`PoolCreationError::NoEnvVar`] when variable did not exist.
fn env_var<K>(key: K) -> Result<String, PoolCreationError>
where
    K: AsRef<OsStr> + Into<String>,
{
    std::env::var(&key).map_err(|err| PoolCreationError::NoEnvVar {
        name: key.into(),
        err,
    })
}
