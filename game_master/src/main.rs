use std::ffi::OsStr;

use backon::{ExponentialBuilder, Retryable as _};
use game_master::{DataSourceImpl, GameMasterServiceImpl};
use struckout_proto::game_master_service_server::GameMasterServiceServer;

use sqlx::{MySql, Pool, mysql::MySqlPoolOptions};
use thiserror::Error;
use tokio::signal::unix::{self, SignalKind};
use tonic::transport::Server;
use tracing::{debug, error, info};
use tracing_subscriber::{layer::SubscriberExt as _, util::SubscriberInitExt as _};

const ENV_MYSQL_ROOT_PASSWORD: &str = "MYSQL_ROOT_PASSWORD";
const ENV_MYSQL_DB_NAME: &str = "MYSQL_DATABASE";

const GRPC_PORT: &str = env!("GAME_MASTER_GRPC_PORT");
#[cfg(feature = "sentry")]
const SENTRY_DSN: &str = env!("SENTRY_DSN");

fn main() -> std::process::ExitCode {
    #[cfg(feature = "sentry")]
    let _guard = sentry::init(
        sentry::ClientOptions::new()
            .dsn(SENTRY_DSN)
            .maybe_release(sentry::release_name!())
            .traces_sample_rate(1.0),
    );

    let reg = tracing_subscriber::registry().with(tracing_subscriber::fmt::layer());
    #[cfg(feature = "sentry")]
    let reg = reg.with(sentry::integrations::tracing::layer());
    reg.init();

    #[cfg(feature = "sentry")]
    info!("initialized tracing with sentry layer");

    let code = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let fut = run_main();
            #[cfg(feature = "sentry")]
            let fut = {
                use sentry::SentryFutureExt;
                fut.bind_hub(sentry::Hub::current())
            };
            fut.await
        });
    std::process::ExitCode::from(code)
}

/// Returns exit code.
async fn run_main() -> u8 {
    info!("creating MySQL pool");
    let pool = match new_pool().await {
        Ok(p) => p,
        Err(err) => {
            error!(?err, "failed to create MySQL pool");
            return 1;
        }
    };
    info!("succeed to create MySQL pool");

    info!("initializing gRPC service");
    let data_source = DataSourceImpl::new(pool);
    let game_master = GameMasterServiceImpl::new(data_source);
    let addr = format!("0.0.0.0:{}", GRPC_PORT)
        .parse()
        .expect("address format should be correct");

    let sig = async {
        let term = unix::signal(SignalKind::terminate());
        let mut term = term.unwrap();
        tokio::select! {
            v = tokio::signal::ctrl_c() => v.unwrap(),
            _ = term.recv() => (),
        }
    };
    match Server::builder()
        .add_service(GameMasterServiceServer::new(game_master))
        .serve_with_shutdown(addr, sig)
        .await
    {
        Ok(_) => (),
        Err(err) => {
            error!(?err, "an error occured");
            return 1;
        }
    };
    return 0;
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

    let opt = MySqlPoolOptions::new();
    let url = format!("mysql://root:{}@db:3306/{}", password, db_name);
    let pool = (|| opt.clone().connect(&url))
        .retry(ExponentialBuilder::default())
        .notify(|err, dur| debug!(?err, ?dur, "retrying connetion"))
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
