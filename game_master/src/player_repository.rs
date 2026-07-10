use sqlx::{Pool, Sqlite};
use thiserror::Error;
use tokio::sync::mpsc;

use crate::worker::WorkerThread;

const MESSAGE_CHANNEL_BUF: usize = 4;

pub struct PlayerRepository {
    tx: mpsc::Sender<PlayerRepositoryMessage>,
}

impl PlayerRepository {
    pub fn new(pool: Pool<Sqlite>, worker: &WorkerThread) -> Self {
        let (tx, mut rx) = mpsc::channel(MESSAGE_CHANNEL_BUF);
        let inner = PlayerRepositoryInner::new(pool);
        worker.spawn(async move {
            loop {
                let msg = rx.recv().await.unwrap();
                match msg {
                    PlayerRepositoryMessage::InsertPlayer(name) => {
                        inner.insert_player(name).await.expect("todo");
                    }
                }
            }
        });
        Self { tx }
    }

    // TODO: oneshotとかでResult受け取りたい
    pub fn insert_player(&self, name: impl Into<String>) {
        self.tx
            .blocking_send(PlayerRepositoryMessage::InsertPlayer(name.into()))
            .unwrap();
    }
}

/// Error type for [`PlayerRepository::insert_player()`].
#[derive(Debug, Error)]
pub enum InsertPlayerError {
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error("Player name {0} is already in use")]
    NameAlredyInUse(String),
}

enum PlayerRepositoryMessage {
    InsertPlayer(String),
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct PlayerEntity {
    id: i64,
    name: String,
}

struct PlayerRepositoryInner {
    pool: Pool<Sqlite>,
}

impl PlayerRepositoryInner {
    fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }

    async fn insert_player(&self, name: impl Into<String>) -> Result<(), InsertPlayerError> {
        let name = name.into();
        let existing =
            sqlx::query_as!(PlayerEntity, "SELECT * FROM players WHERE name == ?", &name)
                .fetch_optional(&self.pool)
                .await?;
        if existing.is_some() {
            return Err(InsertPlayerError::NameAlredyInUse(name.into()));
        }
        sqlx::query!("INSERT INTO players (name) VALUES (?)", &name)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use sqlx::sqlite::SqlitePoolOptions;
    use tempfile::NamedTempFile;

    use super::*;
    #[tokio::test]
    async fn insert_player_returns_error_when_name_already_in_use() {
        let file = NamedTempFile::new().unwrap();

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&file.path().to_string_lossy())
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();

        let player_name = "bob";
        sqlx::query!(
            "INSERT INTO players (id, name) VALUES (?, ?)",
            0,
            player_name
        )
        .execute(&pool)
        .await
        .unwrap();

        let repo = PlayerRepositoryInner::new(pool.clone());
        let res = repo.insert_player(player_name).await;
        assert!(res.is_err());
    }
}
