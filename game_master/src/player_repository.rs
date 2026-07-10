use sqlx::{Pool, Sqlite};
use tokio::sync::mpsc;

pub struct PlayerRepository {
    tx: mpsc::Sender,
}

struct PlayerRepositoryInner {
    pool: Pool<Sqlite>,
}

impl PlayerRepositoryInner {
    fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }

    async fn insert_player(&self, name: impl Into<String>) -> Result<(), sqlx::Error> {
        sqlx::query!("INSERT INTO players (name) VALUES (?)", name.into())
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
