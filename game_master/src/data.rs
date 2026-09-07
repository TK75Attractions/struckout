use std::fmt::Display;

use sqlx::{MySql, Pool};
use time::{PlainDateTime, UtcDateTime};

use crate::{AddPlayerError, DataSource, GameId, MachineId, PlayerId, proto::Difficulty};

#[derive(Clone)]
pub struct DataSourceImpl {
    pool: Pool<MySql>,
}

impl DataSourceImpl {
    pub fn new(pool: Pool<MySql>) -> Self {
        Self { pool }
    }
}

impl DataSource for DataSourceImpl {
    async fn add_player(&self, name: impl Into<String> + Send) -> Result<PlayerId, AddPlayerError> {
        let res = sqlx::query!("INSERT INTO players (name) VALUES (?)", name.into())
            .execute(&self.pool)
            .await
            .map_err(|err| {
                let Some(errdb) = err.as_database_error() else {
                    return err.into();
                };
                // HACK: unique constraint which can be violated by the query above is only 'player.name' for now,
                // but it can be changed in future.
                if errdb.is_unique_violation() {
                    AddPlayerError::NameAlreadyUsed
                } else {
                    err.into()
                }
            })?;

        let last_inserted: u32 = res
            .last_insert_id()
            .try_into()
            .expect("player_id overflowed");
        Ok(last_inserted.into())
    }

    /// Inserts new game into database.
    ///
    /// `started_at` takes [`UtcDateTime`] because it is clearly defined as UTC time and not local time,
    /// but in the function it will be converted to [`PlainDateTime`] since `sqlx`'s MySQL type mapping does not
    /// support [`UtcDateTime`].
    ///
    /// See also: [https://docs.rs/sqlx/latest/sqlx/mysql/types/index.html#time]
    async fn insert_game(
        &self,
        machine_id: MachineId,
        player_id: PlayerId,
        started_at: UtcDateTime,
        difficulty: Difficulty,
    ) -> Result<GameId, sqlx::Error> {
        let machine_id: u32 = machine_id.into();
        let player_id: u32 = player_id.into();
        let started_at = PlainDateTime::new(started_at.date(), started_at.time());
        let res = sqlx::query!(
            "INSERT INTO games (
                machine_id,
                player_id,
                started_at ,
                difficulty,
                status
            ) VALUES (?, ?, ?, ?, 'running')",
            machine_id,
            player_id,
            started_at,
            difficulty.to_string()
        )
        .execute(&self.pool)
        .await?;
        let last_insert_id: u32 = res.last_insert_id().try_into().expect("game_id overflowed");
        Ok(last_insert_id.into())
    }

    /// Completes the game with final score.
    async fn complete_game(&self, game_id: GameId, score: u32) -> Result<(), sqlx::Error> {
        let game_id: u32 = game_id.into();
        sqlx::query!(
            "UPDATE games SET status = 'finished', score = ? WHERE game_id = ?",
            score,
            game_id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

impl Display for Difficulty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Difficulty::Unspecified => "unspecified",
            Difficulty::Normal => "normal",
            Difficulty::Hard => "hard",
            Difficulty::Veryhard => "veryhard",
        };
        write!(f, "{s}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::assert_matches;

    use sqlx::mysql::{MySqlDatabaseError, MySqlPoolOptions};
    use testcontainers_modules::testcontainers::{ContainerAsync, ImageExt, runners::AsyncRunner};
    use time::{Date, Month, Time};

    /// Creates MySQL container, connects to it, and run migration scripts.
    ///
    /// Returned [`ContainerAsync`] must not be dropped while you use it, or you will encounter a [`sqlx::Error::PoolTimedOut`] error.
    async fn init_mysql() -> (
        ContainerAsync<testcontainers_modules::mysql::Mysql>,
        sqlx::Pool<MySql>,
    ) {
        let container = testcontainers_modules::mysql::Mysql::default()
            .with_tag("26.7.0")
            .start()
            .await
            .unwrap();
        let port = container.get_host_port_ipv4(3306).await.unwrap();
        let pool = MySqlPoolOptions::new()
            .connect(format!("mysql://127.0.0.1:{}/test", port).as_str())
            .await
            .expect("failed to connect to MySQL");
        sqlx::migrate!()
            .run(&pool)
            .await
            .expect("failed to run migrations");
        (container, pool)
    }

    #[tokio::test]
    async fn add_player_succeeds() {
        let (_container, pool) = init_mysql().await;

        let ds = DataSourceImpl::new(pool);

        let player_id = ds.add_player("Bob").await.expect("should succeed");

        assert_eq!(player_id, PlayerId(1));
    }

    #[tokio::test]
    async fn add_player_returns_error_when_player_name_already_used() {
        let player_name = "Bob";
        let (_container, pool) = init_mysql().await;

        let ds = DataSourceImpl::new(pool);

        ds.add_player(player_name).await.unwrap();
        let err = ds
            .add_player(player_name)
            .await
            .expect_err("should return error");
        assert_matches!(err, AddPlayerError::NameAlreadyUsed);
    }

    #[tokio::test]
    async fn insert_game_succeeds() {
        let (_container, pool) = init_mysql().await;
        let ds = DataSourceImpl::new(pool);

        let player_id = ds.add_player("Bob").await.expect("failed to insert player");

        let game_id = ds
            .insert_game(
                MachineId(0),
                player_id,
                time::UtcDateTime::new(
                    Date::from_calendar_date(2026, Month::September, 6).unwrap(),
                    Time::from_hms(12, 41, 23).unwrap(),
                ),
                Difficulty::Normal,
            )
            .await
            .expect("should succeed");

        assert_eq!(game_id, GameId(1));
    }

    #[tokio::test]
    async fn complete_game_succeeds() {
        let (_container, pool) = init_mysql().await;
        let ds = DataSourceImpl::new(pool.clone());

        let player_id = ds.add_player("Bob").await.unwrap();

        let game_id = ds
            .insert_game(
                MachineId(0),
                player_id,
                time::UtcDateTime::new(
                    Date::from_calendar_date(2026, Month::September, 6).unwrap(),
                    Time::from_hms(12, 41, 23).unwrap(),
                ),
                Difficulty::Normal,
            )
            .await
            .unwrap();

        const SCORE: u32 = 500;

        ds.complete_game(game_id, SCORE)
            .await
            .expect("should succeed");

        let game_id: u32 = game_id.into();
        let updated = sqlx::query!("SELECT status, score FROM games WHERE game_id = ?", game_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(updated.score.is_some_and(|v| v == SCORE));
        assert_eq!(&updated.status, "finished");
    }
}
