use std::ffi::OsStr;

use backon::{ExponentialBuilder, Retryable as _};
use clap::Parser;
use game_master::{DataSourceImpl, GameMasterPlayerServiceImpl, GameMasterSessionServiceImpl};
use struckout_proto::{
    game_master_player_service_server::GameMasterPlayerServiceServer,
    game_master_session_service_server::GameMasterSessionServiceServer,
};

use sqlx::{MySql, Pool, mysql::MySqlPoolOptions};
use thiserror::Error;
use tokio::{
    signal::unix::{self, SignalKind},
    time::Duration,
};
use tonic::{server::NamedService as _, transport::Server};
use tonic_health::server::health_reporter;
use tracing::{debug, error, info};
use tracing_subscriber::{
    Layer as _,
    filter::{LevelFilter, Targets},
    layer::SubscriberExt as _,
    util::SubscriberInitExt as _,
};

const ENV_MYSQL_ROOT_PASSWORD: &str = "MYSQL_ROOT_PASSWORD";
const ENV_MYSQL_DB_NAME: &str = "MYSQL_DATABASE";

const GRPC_PORT: &str = env!("GAME_MASTER_GRPC_PORT");
#[cfg(feature = "sentry")]
const SENTRY_DSN: &str = "https://2b9af00fb25f2faa692253f8ed95c43e@o4512229597315072.ingest.us.sentry.io/4512229769871360";

type SessionServer = GameMasterSessionServiceServer<GameMasterSessionServiceImpl>;
type PlayerServer = GameMasterSessionServiceServer<GameMasterPlayerServiceImpl>;

#[derive(Parser)]
#[command(version = version())]
struct Cli {}

fn version() -> String {
    if let Some(version) = option_env!("ST_VERSION") {
        let Some(short_hash) = option_env!("ST_COMMIT_SHORT_HASH") else {
            unreachable!("ST_COMMIT_SHORT_HASH should always exist when ST_VERSOPM is provided")
        };
        format!("{} ({})", version, short_hash)
    } else {
        "no version".to_string()
    }
}

fn main() -> std::process::ExitCode {
    #[cfg(feature = "sentry")]
    let _guard = sentry::init(
        sentry::ClientOptions::new()
            .dsn(SENTRY_DSN)
            .maybe_release(sentry::release_name!())
            .traces_sample_rate(1.0),
    );

    // The launcher checks the health every second. Keep the HTTP/2 layers from logging every frame of it.
    let transport_logs = Targets::new()
        .with_default(LevelFilter::TRACE)
        .with_targets([
            ("h2", LevelFilter::INFO),
            ("hyper", LevelFilter::INFO),
            ("tower", LevelFilter::INFO),
            ("tonic", LevelFilter::INFO),
        ]);
    let reg = tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_filter(transport_logs));
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

    // A new database (the first start on a PC, or after its volume is removed) has no tables yet.
    // The migrations are embedded, so a PC that only pulls the image doesn't need sqlx-cli.
    info!("applying migrations");
    if let Err(err) = sqlx::migrate!().run(&pool).await {
        error!(?err, "failed to apply migrations");
        return 1;
    }

    info!("initializing gRPC service");
    let (health_reporter, health_service) = health_reporter();
    let health_task = tokio::spawn({
        let pool = pool.clone();
        game_master::health::monitor_database(
            health_reporter,
            vec![SessionServer::NAME, PlayerServer::NAME],
            move || is_database_available(pool.clone()),
        )
    });
    let data_source = DataSourceImpl::new(pool);
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
    let result = Server::builder()
        .add_service(GameMasterSessionServiceServer::new(
            GameMasterSessionServiceImpl::new(data_source.clone()),
        ))
        .add_service(GameMasterPlayerServiceServer::new(
            GameMasterPlayerServiceImpl::new(data_source),
        ))
        .add_service(health_service)
        .serve_with_shutdown(addr, sig)
        .await;
    health_task.abort();
    match result {
        Ok(_) => (),
        Err(err) => {
            error!(?err, "an error occured");
            return 1;
        }
    };
    return 0;
}

/// Whether the database answers within 2 seconds.
async fn is_database_available(pool: Pool<MySql>) -> bool {
    tokio::time::timeout(
        Duration::from_secs(2),
        sqlx::query("SELECT 1").execute(&pool),
    )
    .await
    .is_ok_and(|result| result.is_ok())
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
