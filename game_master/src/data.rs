use std::{future::Future, ops::DerefMut};

use sqlx::{MySql, MySqlConnection, Pool};
use struckout_proto::{
    Difficulty, GameMode,
    types::{GameId, PlayerId},
};
use time::PlainDateTime;

use crate::{
    AddPlayerError, DataSource, GamePlayers, GameRecord, GetGameResultError, GetPlayerError,
    SaveGameError, ValidatePlayerNameError,
};

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

    /// Saves the game result to the database.
    ///
    /// UTC is converted to PlainDateTime because MySQL's DATETIME has no time zone.
    async fn save_game(&self, game: GameRecord) -> Result<GameId, SaveGameError> {
        let mut conn = self.db.acquire().await?;
        let (machine_id, player_id_1, player_id_2, mode): (Option<u32>, u32, Option<u32>, &str) =
            match game.players {
                GamePlayers::Single {
                    machine_id,
                    player_id,
                } => (Some(machine_id.into()), player_id.into(), None, MODE_SINGLE),
                GamePlayers::Multi((_, first), (_, second)) => {
                    (None, first.into(), Some(second.into()), MODE_MULTI)
                }
            };
        let started_at = PlainDateTime::new(game.started_at.date(), game.started_at.time());
        let res = sqlx::query!(
            "INSERT INTO games (machine_id, player_id_1, player_id_2, started_at, difficulty, score, mode)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            machine_id, player_id_1, player_id_2, started_at,
            game.difficulty.to_mysql_enum(), game.score, mode
        ).execute(&mut *conn).await?;
        let last_inserted: u32 = res.last_insert_id().try_into().expect("game_id overflowed");
        Ok(last_inserted.into())
    }

    /// Reads a persisted result so clients can fetch it after its session was discarded.
    async fn get_game_result(&self, game_id: GameId) -> Result<u32, GetGameResultError> {
        let mut conn = self.db.acquire().await?;
        let row = sqlx::query!(
            "SELECT score FROM games WHERE game_id = ?",
            game_id.into_inner()
        )
        .fetch_optional(&mut *conn)
        .await?
        .ok_or(GetGameResultError::NotFound(game_id))?;
        Ok(row.score)
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
    use struckout_proto::types::MachineId;
    use testcontainers_modules::testcontainers::{ContainerAsync, ImageExt, runners::AsyncRunner};
    use time::{Date, Month, Time, UtcDateTime};
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
            ds.save_game(GameRecord {
                players: GamePlayers::Single {
                    machine_id: MachineId::new(0),
                    player_id,
                },
                started_at: UtcDateTime::now(),
                difficulty: Difficulty::Normal,
                score: 500,
            })
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

    #[rstest::rstest]
    #[case(false)]
    #[case(true)]
    fn save_game_succeeds(#[case] multi: bool) {
        with_test_transaction(|txn| async move {
            let ds = DataSourceImpl::new(txn.clone());
            let first = ds
                .add_player("Charles")
                .await
                .expect("should insert player");
            let second = ds.add_player("David").await.expect("should insert player");
            let players = if multi {
                GamePlayers::Multi((MachineId::new(1), first), (MachineId::new(2), second))
            } else {
                GamePlayers::Single {
                    machine_id: MachineId::new(7),
                    player_id: first,
                }
            };
            let started_at = UtcDateTime::new(
                Date::from_calendar_date(2026, Month::September, 6).unwrap(),
                Time::from_hms(12, 41, 23).unwrap(),
            );
            let game_id = ds
                .save_game(GameRecord {
                    players,
                    started_at,
                    difficulty: Difficulty::Normal,
                    score: 500,
                })
                .await
                .expect("should save final result");
            let row = sqlx::query!("SELECT machine_id, player_id_1, player_id_2, started_at, difficulty, score, mode FROM games WHERE game_id = ?", game_id.into_inner())
                .fetch_one(&mut **txn.lock().await).await.expect("should fetch saved game");
            assert_eq!(row.machine_id, if multi { None } else { Some(7) });
            assert_eq!(row.player_id_1, first.into_inner());
            assert_eq!(
                row.player_id_2,
                if multi {
                    Some(second.into_inner())
                } else {
                    None
                }
            );
            assert_eq!(
                row.started_at,
                PlainDateTime::new(started_at.date(), started_at.time())
            );
            assert_eq!(row.difficulty, "normal");
            assert_eq!(row.score, 500);
            assert_eq!(row.mode, if multi { "multi" } else { "single" });
        });
    }

    #[test]
    fn get_game_result_returns_the_score_for_the_requested_game() {
        with_test_transaction(|txn| async move {
            let ds = DataSourceImpl::new(txn);
            let player_id = ds.add_player("Result player").await.expect("insert player");
            let game = GameRecord {
                players: GamePlayers::Single {
                    machine_id: MachineId::new(1),
                    player_id,
                },
                started_at: UtcDateTime::now(),
                difficulty: Difficulty::Normal,
                score: 123,
            };
            let first = ds.save_game(game.clone()).await.expect("save first game");
            let second = ds
                .save_game(GameRecord { score: 456, ..game })
                .await
                .expect("save second game");

            assert_eq!(ds.get_game_result(first).await.expect("first result"), 123);
            assert_eq!(
                ds.get_game_result(second).await.expect("second result"),
                456
            );
        });
    }

    #[test]
    fn get_game_result_returns_not_found_for_an_unknown_id() {
        with_test_transaction(|txn| async move {
            let ds = DataSourceImpl::new(txn);
            let game_id = GameId::new(u32::MAX);

            let error = ds.get_game_result(game_id).await.expect_err("unknown game");

            assert_matches!(error, GetGameResultError::NotFound(id) if id == game_id);
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
