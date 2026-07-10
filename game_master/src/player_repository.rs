use sqlx::{Pool, Sqlite};
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

enum PlayerRepositoryMessage {
    InsertPlayer(String),
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
