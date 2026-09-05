use std::ffi::OsStr;

use sqlx::{MySql, Pool, mysql::MySqlPoolOptions};
use thiserror::Error;
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

    info!("creating MySQL pool");
    let pool = match new_pool().await {
        Ok(p) => p,
        Err(err) => {
            error!(?err, "failed to create MySQL pool");
            std::process::exit(1);
        }
    };
    info!("succeed to create MySQL pool");
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
