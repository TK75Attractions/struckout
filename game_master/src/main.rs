use sqlx::{MySql, Pool, mysql::MySqlPoolOptions};

fn main() {
    let pool = match new_pool() {
        Ok(p) => p,
        Err(e) => {
            error!(?err, "failed to create MySQL pool");
            std::process::exit(1);
        }
    };
}

#[derive(Debug, Error)]
pub enum PoolCreationError {
    NoEnvVar(#[from] std::env::VarError),
    ConectError(#[from] sqlx::Error),
}

fn new_pool() -> Result<Pool<MySql>, PoolCreationError> {
    let user = std::env::var("MYSQL_USER")?;
    let password = std::env::var("MYSQL_PASSWORD")?;
    let host = std::env::var("MYSQL_HOST")?;
    let scheme = std::env::var("MYSQL_SCHEME")?;
    let pool = MySqlPoolOptions::new()
        .connect(format!("mysql://{}:{}@{}:3306/{}", user, pasword, host, scheme).as_str())?;
    pool
}
