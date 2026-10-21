use std::{future::Future, ops::DerefMut};

use sqlx::{MySql, MySqlConnection, Pool};
use struckout_proto::{
    Difficulty, GameMode,
    types::{GameId, MachineId, PlayerId},
};
use time::{PlainDateTime, UtcDateTime};

use crate::{
    AddPlayerError, DataSource, GetGameResultError, GetPlayerError, ValidatePlayerNameError,
};

const STATUS_FINISHED: &str = "finished";
const STATUS_RUNNING: &str = "running";

const MODE_SINGLE: &str = "single";
const MODE_MULTI: &str = "multi";

#[derive(Clone)]
pub struct DataSourceImpl<T = Pool<MySql>> {
    db: T,
}

impl<T> DataSourceImpl<T> {
    pub fn new(db: T) -> Self {
        Self { db }
    }
}

/// Acquires connections separately from queries so tests can use a transaction.
trait MySqlConnectionSource: Clone + Send + Sync + 'static {
    fn acquire(
        &self,
    ) -> impl Future<Output = Result<impl DerefMut<Target = MySqlConnection> + Send, sqlx::Error>> + Send;
}

impl MySqlConnectionSource for Pool<MySql> {
    async fn acquire(&self) -> Result<impl DerefMut<Target = MySqlConnection> + Send, sqlx::Error> {
        self.acquire().await
    }
}

impl<C: MySqlConnectionSource> DataSource for DataSourceImpl<C> {
    async fn add_player(&self, name: impl Into<String> + Send) -> Result<PlayerId, AddPlayerError> {
        let mut conn = self.db.acquire().await?;
        let res = sqlx::query!("INSERT INTO players (name) VALUES (?)", name.into())
            .execute(&mut *conn)
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
    async fn insert_game_single(
        &self,
        machine_id: MachineId,
        player_id: PlayerId,
        started_at: UtcDateTime,
        difficulty: Difficulty,
    ) -> Result<GameId, sqlx::Error> {
        let mut conn = self.db.acquire().await?;
        let machine_id: u32 = machine_id.into();
        let player_id: u32 = player_id.into();
        let started_at = PlainDateTime::new(started_at.date(), started_at.time());
        let res = sqlx::query!(
            "INSERT INTO games (
                machine_id,
                player_id_1,
                started_at ,
                difficulty,
                status,
                mode
            ) VALUES (?, ?, ?, ?, ?, ?)",
            machine_id,
            player_id,
            started_at,
            difficulty.to_mysql_enum(),
            STATUS_RUNNING,
            MODE_SINGLE
        )
        .execute(&mut *conn)
        .await?;
        let last_insert_id: u32 = res.last_insert_id().try_into().expect("game_id overflowed");
        Ok(last_insert_id.into())
    }

    /// Inserts new game into database.
    ///
    /// `started_at` takes [`UtcDateTime`] because it is clearly defined as UTC time and not local time,
    /// but in the function it will be converted to [`PlainDateTime`] since `sqlx`'s MySQL type mapping does not
    /// support [`UtcDateTime`].
    ///
    /// See also: [https://docs.rs/sqlx/latest/sqlx/mysql/types/index.html#time]
    async fn insert_game_multi(
        &self,
        player_ids: [PlayerId; 2],
        started_at: UtcDateTime,
        difficulty: Difficulty,
    ) -> Result<GameId, sqlx::Error> {
        let mut conn = self.db.acquire().await?;
        let player_id_1: u32 = player_ids[0].into();
        let player_id_2: u32 = player_ids[1].into();
        let started_at = PlainDateTime::new(started_at.date(), started_at.time());
        let res = sqlx::query!(
            "INSERT INTO games (
                player_id_1,
                player_id_2,
                started_at,
                difficulty,
                status,
                mode
            ) VALUES (?, ?, ?, ?, ?, ?)",
            player_id_1,
            player_id_2,
            started_at,
            difficulty.to_mysql_enum(),
            STATUS_RUNNING,
            MODE_MULTI,
        )
        .execute(&mut *conn)
        .await?;
        let last_insert_id: u32 = res.last_insert_id().try_into().expect("game_id overflowed");
        Ok(last_insert_id.into())
    }

    /// Completes the game with final score.
    async fn complete_game(&self, game_id: GameId, score: u32) -> Result<(), sqlx::Error> {
        let mut conn = self.db.acquire().await?;
        let game_id: u32 = game_id.into();
        sqlx::query!(
            "UPDATE games SET status = 'finished', score = ? WHERE game_id = ?",
            score,
            game_id
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    /// Gets the result of a completed game.
    async fn get_game_result(&self, game_id: GameId) -> Result<GameRecord, GetGameResultError> {
        let mut conn = self.db.acquire().await?;
        let row = sqlx::query!(
            "SELECT score, status FROM games WHERE game_id = ?",
            game_id.into_inner()
        )
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => GetGameResultError::GameNotFound,
            _ => e.into(),
        })?;

        if row.status != STATUS_FINISHED {
            return Err(GetGameResultError::NotYetCompleted);
        }
        // guaranteed by CHECK constraint
        let score = row.score.unwrap();

        Ok(GameRecord { score })
    }

    async fn validate_player_name(
        &self,
        name: impl Into<String> + Send,
    ) -> Result<(), ValidatePlayerNameError> {
        let mut conn = self.db.acquire().await?;
        let name = name.into();
        let res = sqlx::query!("SELECT * FROM players WHERE name = ?", name)
            .fetch_optional(&mut *conn)
            .await?;
        match res {
            Some(v) => Err(ValidatePlayerNameError::AlreadyUsed(v.name)),
            None => Ok(()),
        }
    }

    async fn get_player(&self, name: impl Into<String> + Send) -> Result<Player, GetPlayerError> {
        let mut conn = self.db.acquire().await?;
        let name = name.into();
        let res = sqlx::query!("SELECT * FROM players WHERE name = ?", &name)
            .fetch_optional(&mut *conn)
            .await?;
        match res {
            Some(r) => Ok(Player {
                player_id: r.id.into(),
                name,
            }),
            None => Err(GetPlayerError::NotExist(name)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct GameRecord {
    pub score: u32,
}

#[derive(Debug, Clone)]
pub struct Player {
    pub player_id: PlayerId,
    pub name: String,
}

/// The type implementing this trait can be converted from/into MySQL's enum column.
trait MySqlEnum {
    /// Convert type to MySQL's enum type.
    ///
    /// Returns `None` if `self`'s value is not part of defined values (e.g. UNSPECIFIED).
    fn to_mysql_enum(&self) -> Option<&str>;

    /// Convert raw string from database into a domain type.
    ///
    /// Returns `None` when the value is invalid.
    #[allow(dead_code)] // Possibly we would convert a fetched content to domain type in future.
    fn from_mysql_enum(s: &str) -> Option<Self>
    where
        Self: Sized;
}

/// Possible enum values in database are defined in `{repo_root}/migrations/mysql/`.
const MYSQL_DIFFICULTY_NORMAL: &str = "normal";
/// Possible enum values in database are defined in `{repo_root}/migrations/mysql/`.
const MYSQL_DIFFICULTY_HARD: &str = "hard";
/// Possible enum values in database are defined in `{repo_root}/migrations/mysql/`.
const MYSQL_DIFFICULTY_VERYHARD: &str = "veryhard";

impl MySqlEnum for Difficulty {
    fn to_mysql_enum(&self) -> Option<&str> {
        match self {
            Difficulty::Unspecified => None,
            Difficulty::Normal => Some(MYSQL_DIFFICULTY_NORMAL),
            Difficulty::Hard => Some(MYSQL_DIFFICULTY_HARD),
            Difficulty::Veryhard => Some(MYSQL_DIFFICULTY_VERYHARD),
        }
    }

    fn from_mysql_enum(s: &str) -> Option<Self> {
        match s {
            MYSQL_DIFFICULTY_NORMAL => Some(Self::Normal),
            MYSQL_DIFFICULTY_HARD => Some(Self::Hard),
            MYSQL_DIFFICULTY_VERYHARD => Some(Self::Veryhard),
            _ => None,
        }
    }
}

impl MySqlEnum for GameMode {
    fn to_mysql_enum(&self) -> Option<&str> {
        match self {
            GameMode::Unspecified => None,
            GameMode::Single => Some(MODE_SINGLE),
            GameMode::Multi => Some(MODE_MULTI),
        }
    }

    fn from_mysql_enum(s: &str) -> Option<Self>
    where
        Self: Sized,
    {
        match s {
            MODE_SINGLE => Some(GameMode::Single),
            MODE_MULTI => Some(GameMode::Multi),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::{
        assert_matches,
        sync::{Arc, LazyLock},
    };

    use parking_lot::Mutex;
    use sqlx::mysql::MySqlPoolOptions;
    use testcontainers_modules::testcontainers::{ContainerAsync, ImageExt, runners::AsyncRunner};
    use time::{Date, Month, Time};
    use tokio::{runtime::Runtime, sync::Mutex as TokioMutex};

    type TestTransaction = Arc<TokioMutex<sqlx::Transaction<'static, MySql>>>;

    impl MySqlConnectionSource for TestTransaction {
        async fn acquire(
            &self,
        ) -> Result<impl DerefMut<Target = MySqlConnection> + Send, sqlx::Error> {
            Ok(tokio::sync::MutexGuard::map(self.lock().await, |tx| {
                &mut **tx
            }))
        }
    }

    // Keep the runtime alive alongside the shared pool and Docker client.
    static RUNTIME: LazyLock<Runtime> = LazyLock::new(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("failed to create database test runtime")
    });
    static MYSQL: Mutex<Option<TestDatabase>> = Mutex::new(None);

    struct TestDatabase {
        container: ContainerAsync<testcontainers_modules::mysql::Mysql>,
        pool: Pool<MySql>,
    }

    /// Serializes database tests and rolls back their changes before releasing the lock.
    fn with_test_transaction<F, Fut>(test: F)
    where
        F: FnOnce(TestTransaction) -> Fut,
        Fut: Future<Output = ()>,
    {
        let mut database = MYSQL.lock();
        RUNTIME.block_on(async {
            if database.is_none() {
                *database = Some(init_mysql().await);
            }
            let database = database.as_ref().expect("database was initialized above");
            let transaction = Arc::new(TokioMutex::new(
                database
                    .pool
                    .begin()
                    .await
                    .expect("failed to begin test transaction"),
            ));
            // Dropping the transaction also rolls back if the test panics.
            test(transaction.clone()).await;
            Arc::try_unwrap(transaction)
                .expect("test must release all transaction references")
                .into_inner()
                .rollback()
                .await
                .expect("failed to roll back test transaction");
        });
    }

    // Static values are not dropped at process exit, so explicitly remove the container.
    dtor::declarative::dtor! {
        #[dtor]
        unsafe fn cleanup_mysql() {
            if let Some(database) = MYSQL.lock().take() {
                RUNTIME.block_on(async {
                    database.pool.close().await;
                    database.container.rm().await.expect("failed to remove test MySQL container");
                });
            }
        }
    }

    /// Creates the shared MySQL container and runs migrations once per test process.
    async fn init_mysql() -> TestDatabase {
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
        TestDatabase { container, pool }
    }

    #[test]
    fn test_transactions_rollback_changes() {
        with_test_transaction(|transaction| async move {
            let ds = DataSourceImpl::new(transaction);
            let player_id = ds
                .add_player("Rollback")
                .await
                .expect("should insert player");
            ds.insert_game_single(
                MachineId::new(0),
                player_id,
                UtcDateTime::now(),
                Difficulty::Normal,
            )
            .await
            .expect("should insert game");
        });

        with_test_transaction(|transaction| async move {
            let mut connection = transaction
                .acquire()
                .await
                .expect("should acquire connection");
            macro_rules! assert_count_zero {
                ($query:literal) => {
                    let count: i64 = sqlx::query_scalar!($query)
                        .fetch_one(&mut *connection)
                        .await
                        .expect("should count rows");
                    assert_eq!(count, 0, "tables must be empty after rollback: {}", $query);
                };
            }
            assert_count_zero!("SELECT COUNT(*) FROM games");
            assert_count_zero!("SELECT COUNT(*) FROM players");
        });
    }

    #[test]
    fn test_transactions_rollback_on_panic() {
        let result = std::panic::catch_unwind(|| {
            with_test_transaction(|transaction| async move {
                let ds = DataSourceImpl::new(transaction);
                ds.add_player("Rollback after panic")
                    .await
                    .expect("should insert player");
                panic!("simulate a failed test");
            });
        });
        assert!(result.is_err());

        with_test_transaction(|transaction| async move {
            let ds = DataSourceImpl::new(transaction);
            ds.validate_player_name("Rollback after panic")
                .await
                .expect("the failed test must not leave the player behind");
        });
    }

    #[test]
    fn add_player_succeeds() {
        with_test_transaction(|txn| async move {
            let ds = DataSourceImpl::new(txn.clone());

            let player_id = ds.add_player("Bob").await.expect("should succeed");

            let player = ds.get_player("Bob").await.expect("player should exist");
            assert_eq!(player.player_id, player_id);
        });
    }

    #[test]
    fn add_player_returns_error_when_player_name_already_used() {
        with_test_transaction(|txn| async move {
            let player_name = "Bob";

            let ds = DataSourceImpl::new(txn.clone());

            ds.add_player(player_name).await.unwrap();
            let err = ds
                .add_player(player_name)
                .await
                .expect_err("should return error");
            assert_matches!(err, AddPlayerError::NameAlreadyUsed);
        });
    }

    #[test]
    fn insert_game_single_succeeds() {
        with_test_transaction(|txn| async move {
            let ds = DataSourceImpl::new(txn.clone());

            let player_id = ds.add_player("Bob").await.expect("failed to insert player");

            let game_id = ds
                .insert_game_single(
                    MachineId::new(0),
                    player_id,
                    time::UtcDateTime::new(
                        Date::from_calendar_date(2026, Month::September, 6).unwrap(),
                        Time::from_hms(12, 41, 23).unwrap(),
                    ),
                    Difficulty::Normal,
                )
                .await
                .expect("should succeed");

            let exists: bool = sqlx::query_scalar!(
                "SELECT EXISTS(SELECT 1 FROM games WHERE game_id = ?) AS `exists: bool`",
                game_id.into_inner()
            )
            .fetch_one(&mut **txn.lock().await)
            .await
            .expect("failed to fetch inserted game");
            assert!(exists);
        });
    }

    #[test]
    fn insert_game_multi_succeeds() {
        with_test_transaction(|txn| async move {
            let ds = DataSourceImpl::new(txn.clone());

            let player_id_1 = ds
                .add_player("Charles")
                .await
                .expect("failed to insert player");
            let player_id_2 = ds
                .add_player("David")
                .await
                .expect("failed to insert player");

            let game_id = ds
                .insert_game_multi(
                    [player_id_1, player_id_2],
                    time::UtcDateTime::new(
                        Date::from_calendar_date(2026, Month::September, 6).unwrap(),
                        Time::from_hms(12, 41, 23).unwrap(),
                    ),
                    Difficulty::Normal,
                )
                .await
                .expect("should succeed");

            let exists: bool = sqlx::query_scalar!(
                "SELECT EXISTS(SELECT 1 FROM games WHERE game_id = ?) AS `exists: bool`",
                game_id.into_inner()
            )
            .fetch_one(&mut **txn.lock().await)
            .await
            .expect("failed to fetch inserted game");
            assert!(exists);
        });
    }

    #[test]
    fn complete_game_succeeds() {
        with_test_transaction(|txn| async move {
            let ds = DataSourceImpl::new(txn.clone());

            let player_id = ds.add_player("Bob").await.unwrap();

            let game_id = ds
                .insert_game_single(
                    MachineId::new(0),
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
            let updated =
                sqlx::query!("SELECT status, score FROM games WHERE game_id = ?", game_id)
                    .fetch_one(&mut **txn.lock().await)
                    .await
                    .unwrap();
            assert!(updated.score.is_some_and(|v| v == SCORE));
            assert_eq!(&updated.status, "finished");
        });
    }

    #[test]
    fn validate_player_name_returns_ok() {
        with_test_transaction(|transaction| async move {
            let name = "テスタロウ";

            let ds = DataSourceImpl::new(transaction.clone());

            ds.validate_player_name(name)
                .await
                .expect("should return ok");
        });
    }

    #[test]
    fn validate_player_name_returns_err_when_name_already_used() {
        with_test_transaction(|transaction| async move {
            let name = "タロウ";

            let ds = DataSourceImpl::new(transaction.clone());

            sqlx::query!("INSERT INTO players (name) VALUES (?)", name)
                .execute(&mut **transaction.lock().await)
                .await
                .unwrap();

            let err = ds
                .validate_player_name(name)
                .await
                .expect_err("should return error");
            assert_matches!(err, ValidatePlayerNameError::AlreadyUsed(v) if v == name);
        });
    }

    #[test]
    fn get_plater_returns_err_when_player_not_exist() {
        with_test_transaction(|transaction| async move {
            let ds = DataSourceImpl::new(transaction.clone());

            let res = ds.get_player("アムロ").await;
            assert_matches!(res, Err(GetPlayerError::NotExist(_)));
        });
    }

    #[test]
    fn get_player_returns_ok_when_player_exist() {
        with_test_transaction(|transaction| async move {
            let ds = DataSourceImpl::new(transaction.clone());
            let name = "カミーユ";

            sqlx::query!("INSERT INTO players (id, name) VALUES (?, ?)", 1, name)
                .execute(&mut **transaction.lock().await)
                .await
                .unwrap();

            let res = ds.get_player(name).await;
            assert_matches!(res, Ok(Player { player_id:pid_got, name:name_got }) if pid_got == PlayerId::new(1) && name_got == name);
        });
    }
}
