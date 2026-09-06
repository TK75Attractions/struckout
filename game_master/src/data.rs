use std::fmt::Display;

use sqlx::MySql;
use time::{PlainDateTime, UtcDateTime};

use crate::{GameId, MachineId, PlayerId, proto::Difficulty};

pub async fn insert_player(
    pool: &sqlx::Pool<MySql>,
    name: impl Into<String>,
) -> Result<PlayerId, sqlx::Error> {
    let res = sqlx::query!("INSERT INTO players (name) VALUES (?)", name.into())
        .execute(pool)
        .await?;
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
pub async fn insert_game(
    pool: &sqlx::Pool<MySql>,
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
    .execute(pool)
    .await?;
    let last_insert_id: u32 = res.last_insert_id().try_into().expect("game_id overflowed");
    Ok(last_insert_id.into())
}

/// Completes the game with final score.
pub async fn complete_game(
    pool: &sqlx::Pool<MySql>,
    game_id: GameId,
    score: u32,
) -> Result<(), sqlx::Error> {
    let game_id: u32 = game_id.into();
    sqlx::query!(
        "UPDATE games SET status = 'finished', score = ? WHERE game_id = ?",
        score,
        game_id
    )
    .execute(pool)
    .await?;
    Ok(())
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
    use sqlx::{MySql, mysql::MySqlPoolOptions};
    use testcontainers_modules::testcontainers::{ContainerAsync, ImageExt, runners::AsyncRunner};
    use time::{Date, Month, Time};

    use crate::{
        GameId, MachineId, PlayerId,
        data::{complete_game, insert_game, insert_player},
        proto::Difficulty,
    };

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
    async fn insert_player_succeeds() {
        let (_container, pool) = init_mysql().await;

        let player_id = insert_player(&pool, "Bob").await.expect("should succeed");

        assert_eq!(player_id, PlayerId(1));
    }

    #[tokio::test]
    async fn insert_game_succeeds() {
        let (_container, pool) = init_mysql().await;

        let player_id = insert_player(&pool, "Bob")
            .await
            .expect("failed to insert player");

        let game_id = insert_game(
            &pool,
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

        let player_id = insert_player(&pool, "Bob").await.unwrap();

        let game_id = insert_game(
            &pool,
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

        complete_game(&pool, game_id, SCORE)
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
