#[cfg(not(any(
    feature = "persistence-diesel",
    feature = "persistence-sqlx",
    feature = "persistence-seaorm"
)))]
compile_error!("enable exactly one persistence backend feature");

#[cfg(any(
    all(feature = "persistence-diesel", feature = "persistence-sqlx"),
    all(feature = "persistence-diesel", feature = "persistence-seaorm"),
    all(feature = "persistence-sqlx", feature = "persistence-seaorm")
))]
compile_error!("enable exactly one persistence backend feature");

use async_trait::async_trait;
use snafu::ResultExt;

use crate::{
    config::TargetConfig,
    error::{ParseStateSnafu, Result, SerializeStateSnafu},
    models::{CheckOutcome, EngineUsed, TargetStatus},
};

const SCHEMA_VERSION: i64 = 1;
const DROP_TABLES: [&str; 3] = [
    "DROP TABLE IF EXISTS checks",
    "DROP TABLE IF EXISTS target_state",
    "DROP TABLE IF EXISTS targets",
];
const CREATE_TABLES: [&str; 3] = [
    "CREATE TABLE IF NOT EXISTS targets (id TEXT PRIMARY KEY, name TEXT NOT NULL, url TEXT NOT NULL, enabled INTEGER NOT NULL DEFAULT 1, conditions_json TEXT NOT NULL, updated_at TEXT NOT NULL)",
    "CREATE TABLE IF NOT EXISTS target_state (target_id TEXT PRIMARY KEY REFERENCES targets(id) ON DELETE CASCADE, matched INTEGER, engine_used TEXT, price_cents INTEGER, evidence_json TEXT NOT NULL DEFAULT '[]', condition_results_json TEXT NOT NULL DEFAULT '[]', last_success_at TEXT, last_error_at TEXT, last_error TEXT, last_alert_at TEXT)",
    "CREATE TABLE IF NOT EXISTS checks (id INTEGER PRIMARY KEY AUTOINCREMENT, target_id TEXT NOT NULL REFERENCES targets(id) ON DELETE CASCADE, checked_at TEXT NOT NULL, matched INTEGER, engine_used TEXT, price_cents INTEGER, evidence_json TEXT NOT NULL DEFAULT '[]', condition_results_json TEXT NOT NULL DEFAULT '[]', error TEXT)",
];
const STATUS_SQL: &str = "SELECT t.id, t.name, t.url, s.matched, s.engine_used, s.price_cents, s.evidence_json, s.condition_results_json, s.last_success_at, s.last_error_at, s.last_error, s.last_alert_at FROM targets t LEFT JOIN target_state s ON s.target_id = t.id ORDER BY t.id";

#[async_trait]
pub trait Persistence: Send + Sync {
    async fn migrate(&self) -> Result<()>;
    async fn ensure_target(&self, target: &TargetConfig) -> Result<()>;
    async fn record_success(&self, outcome: &CheckOutcome) -> Result<bool>;
    async fn record_error(&self, target_id: &str, error: &str) -> Result<()>;
    async fn mark_alert_sent(&self, target_id: &str) -> Result<()>;
    async fn statuses(&self) -> Result<Vec<TargetStatus>>;
    async fn status(&self, target_id: &str) -> Result<Option<TargetStatus>> {
        Ok(self
            .statuses()
            .await?
            .into_iter()
            .find(|status| status.target_id == target_id))
    }
}

pub async fn connect(path: &str) -> Result<Box<dyn Persistence>> {
    active::connect(path).await
}

pub fn backend_name() -> &'static str {
    active::BACKEND_NAME
}

#[cfg(all(
    feature = "persistence-diesel",
    not(feature = "persistence-sqlx"),
    not(feature = "persistence-seaorm")
))]
mod active {
    use super::*;
    use diesel::{
        prelude::*,
        r2d2::{ConnectionManager, Pool},
        sql_query, QueryableByName,
    };

    pub const BACKEND_NAME: &str = "diesel";
    type DieselPool = Pool<ConnectionManager<SqliteConnection>>;

    pub async fn connect(path: &str) -> Result<Box<dyn Persistence>> {
        let path = path.to_string();
        let backend = spawn(move || {
            let manager = ConnectionManager::<SqliteConnection>::new(path);
            Pool::builder()
                .build(manager)
                .map(|pool| DieselPersistence { pool })
                .map_err(db_err)
        })
        .await?;
        Ok(Box::new(backend))
    }

    struct DieselPersistence {
        pool: DieselPool,
    }

    #[derive(QueryableByName)]
    struct PragmaRow {
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        user_version: i64,
    }

    #[derive(QueryableByName)]
    struct MatchedRow {
        #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::BigInt>)]
        matched: Option<i64>,
    }

    #[derive(QueryableByName)]
    struct StatusRow {
        #[diesel(sql_type = diesel::sql_types::Text)]
        id: String,
        #[diesel(sql_type = diesel::sql_types::Text)]
        name: String,
        #[diesel(sql_type = diesel::sql_types::Text)]
        url: String,
        #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::BigInt>)]
        matched: Option<i64>,
        #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
        engine_used: Option<String>,
        #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::BigInt>)]
        price_cents: Option<i64>,
        #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
        evidence_json: Option<String>,
        #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
        condition_results_json: Option<String>,
        #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
        last_success_at: Option<String>,
        #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
        last_error_at: Option<String>,
        #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
        last_error: Option<String>,
        #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
        last_alert_at: Option<String>,
    }

    #[async_trait]
    impl Persistence for DieselPersistence {
        async fn migrate(&self) -> Result<()> {
            let pool = self.pool.clone();
            spawn(move || {
                let conn = &mut conn(&pool)?;
                let version = sql_query("PRAGMA user_version")
                    .load::<PragmaRow>(conn)
                    .map_err(db_err)?
                    .into_iter()
                    .next()
                    .map(|row| row.user_version)
                    .unwrap_or(0);
                if version != SCHEMA_VERSION {
                    for sql in DROP_TABLES {
                        sql_query(sql).execute(conn).map_err(db_err)?;
                    }
                }
                for sql in CREATE_TABLES {
                    sql_query(sql).execute(conn).map_err(db_err)?;
                }
                sql_query(format!("PRAGMA user_version = {SCHEMA_VERSION}"))
                    .execute(conn)
                    .map_err(db_err)?;
                Ok(())
            })
            .await
        }

        async fn ensure_target(&self, target: &TargetConfig) -> Result<()> {
            let pool = self.pool.clone();
            let target = target.clone();
            spawn(move || {
                use diesel::sql_types::{BigInt, Text};
                let conn = &mut conn(&pool)?;
                let parsed = target.to_target()?;
                let enabled = i64::from(target.enabled());
                let now = chrono::Utc::now().to_rfc3339();
                let conditions_json =
                    serde_json::to_string(&parsed.conditions).context(SerializeStateSnafu)?;
                sql_query("INSERT INTO targets (id, name, url, enabled, conditions_json, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6) ON CONFLICT(id) DO UPDATE SET name = excluded.name, url = excluded.url, enabled = excluded.enabled, conditions_json = excluded.conditions_json, updated_at = excluded.updated_at")
                    .bind::<Text, _>(&target.id)
                    .bind::<Text, _>(&target.name)
                    .bind::<Text, _>(&parsed.url)
                    .bind::<BigInt, _>(enabled)
                    .bind::<Text, _>(&conditions_json)
                    .bind::<Text, _>(&now)
                    .execute(conn)
                    .map_err(db_err)?;
                sql_query("INSERT OR IGNORE INTO target_state (target_id) VALUES (?1)")
                    .bind::<Text, _>(&target.id)
                    .execute(conn)
                    .map_err(db_err)?;
                Ok(())
            })
            .await
        }

        async fn record_success(&self, outcome: &CheckOutcome) -> Result<bool> {
            let pool = self.pool.clone();
            let outcome = outcome.clone();
            spawn(move || {
                use diesel::sql_types::{BigInt, Nullable, Text};
                let conn = &mut conn(&pool)?;
                let target_id = outcome.target.id.clone();
                let was_matched = was_matched(conn, &target_id)?;
                let should_alert = outcome.condition_met() && !was_matched;
                let checked_at = outcome.checked_at.to_rfc3339();
                let evidence_json = serde_json::to_string(&outcome.evidence)
                    .context(SerializeStateSnafu)?;
                let condition_results_json = serde_json::to_string(&outcome.condition_results)
                    .context(SerializeStateSnafu)?;
                let engine = engine_to_str(outcome.engine_used);
                sql_query("INSERT INTO checks (target_id, checked_at, matched, engine_used, price_cents, evidence_json, condition_results_json, error) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL)")
                    .bind::<Text, _>(&target_id)
                    .bind::<Text, _>(&checked_at)
                    .bind::<BigInt, _>(i64::from(outcome.matched))
                    .bind::<Text, _>(engine)
                    .bind::<Nullable<BigInt>, _>(outcome.price_cents)
                    .bind::<Text, _>(&evidence_json)
                    .bind::<Text, _>(&condition_results_json)
                    .execute(conn)
                    .map_err(db_err)?;
                sql_query("UPDATE target_state SET matched = ?2, engine_used = ?3, price_cents = ?4, evidence_json = ?5, condition_results_json = ?6, last_success_at = ?7, last_error_at = NULL, last_error = NULL WHERE target_id = ?1")
                    .bind::<Text, _>(&target_id)
                    .bind::<BigInt, _>(i64::from(outcome.matched))
                    .bind::<Text, _>(engine)
                    .bind::<Nullable<BigInt>, _>(outcome.price_cents)
                    .bind::<Text, _>(&evidence_json)
                    .bind::<Text, _>(&condition_results_json)
                    .bind::<Text, _>(&checked_at)
                    .execute(conn)
                    .map_err(db_err)?;
                Ok(should_alert)
            })
            .await
        }

        async fn record_error(&self, target_id: &str, error: &str) -> Result<()> {
            let pool = self.pool.clone();
            let target_id = target_id.to_string();
            let error = error.to_string();
            spawn(move || {
                use diesel::sql_types::Text;
                let conn = &mut conn(&pool)?;
                let now = chrono::Utc::now().to_rfc3339();
                sql_query("INSERT INTO checks (target_id, checked_at, error) VALUES (?1, ?2, ?3)")
                    .bind::<Text, _>(&target_id)
                    .bind::<Text, _>(&now)
                    .bind::<Text, _>(&error)
                    .execute(conn)
                    .map_err(db_err)?;
                sql_query("UPDATE target_state SET last_error_at = ?2, last_error = ?3 WHERE target_id = ?1")
                    .bind::<Text, _>(&target_id)
                    .bind::<Text, _>(&now)
                    .bind::<Text, _>(&error)
                    .execute(conn)
                    .map_err(db_err)?;
                Ok(())
            })
            .await
        }

        async fn mark_alert_sent(&self, target_id: &str) -> Result<()> {
            let pool = self.pool.clone();
            let target_id = target_id.to_string();
            spawn(move || {
                use diesel::sql_types::Text;
                let conn = &mut conn(&pool)?;
                let now = chrono::Utc::now().to_rfc3339();
                sql_query("UPDATE target_state SET last_alert_at = ?2 WHERE target_id = ?1")
                    .bind::<Text, _>(&target_id)
                    .bind::<Text, _>(&now)
                    .execute(conn)
                    .map_err(db_err)?;
                Ok(())
            })
            .await
        }

        async fn statuses(&self) -> Result<Vec<TargetStatus>> {
            let pool = self.pool.clone();
            spawn(move || {
                let conn = &mut conn(&pool)?;
                sql_query(STATUS_SQL)
                    .load::<StatusRow>(conn)
                    .map_err(db_err)?
                    .into_iter()
                    .map(|row| {
                        status_from_parts(StatusParts {
                            id: row.id,
                            name: row.name,
                            url: row.url,
                            matched: row.matched,
                            engine_used: row.engine_used,
                            price_cents: row.price_cents,
                            evidence_json: row.evidence_json,
                            condition_results_json: row.condition_results_json,
                            last_success_at: row.last_success_at,
                            last_error_at: row.last_error_at,
                            last_error: row.last_error,
                            last_alert_at: row.last_alert_at,
                        })
                    })
                    .collect()
            })
            .await
        }
    }

    fn conn(
        pool: &DieselPool,
    ) -> Result<diesel::r2d2::PooledConnection<ConnectionManager<SqliteConnection>>> {
        pool.get().map_err(db_err)
    }

    fn was_matched(conn: &mut SqliteConnection, target_id: &str) -> Result<bool> {
        use diesel::sql_types::Text;
        Ok(
            sql_query("SELECT matched FROM target_state WHERE target_id = ?1")
                .bind::<Text, _>(target_id)
                .load::<MatchedRow>(conn)
                .map_err(db_err)?
                .into_iter()
                .next()
                .and_then(|row| row.matched)
                .unwrap_or(0)
                != 0,
        )
    }

    async fn spawn<T, F>(f: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce() -> Result<T> + Send + 'static,
    {
        tokio::task::spawn_blocking(f)
            .await
            .map_err(|error| crate::Error::PersistenceTask {
                message: error.to_string(),
            })?
    }

    fn db_err(error: impl std::fmt::Display) -> crate::Error {
        crate::Error::Database {
            message: error.to_string(),
        }
    }
}

#[cfg(all(
    feature = "persistence-sqlx",
    not(feature = "persistence-diesel"),
    not(feature = "persistence-seaorm")
))]
mod active {
    use super::*;
    use sqlx::{sqlite::SqliteConnectOptions, Row, SqlitePool};
    use std::str::FromStr;

    pub const BACKEND_NAME: &str = "sqlx";

    pub async fn connect(path: &str) -> Result<Box<dyn Persistence>> {
        let options = SqliteConnectOptions::from_str(path)
            .or_else(|_| SqliteConnectOptions::from_str(&format!("sqlite://{path}")))
            .map_err(db_err)?
            .create_if_missing(true);
        let pool = SqlitePool::connect_with(options).await.map_err(db_err)?;
        Ok(Box::new(SqlxPersistence { pool }))
    }

    struct SqlxPersistence {
        pool: SqlitePool,
    }

    #[async_trait]
    impl Persistence for SqlxPersistence {
        async fn migrate(&self) -> Result<()> {
            let version: i64 = sqlx::query_scalar("PRAGMA user_version")
                .fetch_one(&self.pool)
                .await
                .map_err(db_err)?;
            if version != SCHEMA_VERSION {
                for sql in DROP_TABLES {
                    sqlx::query(sql).execute(&self.pool).await.map_err(db_err)?;
                }
            }
            for sql in CREATE_TABLES {
                sqlx::query(sql).execute(&self.pool).await.map_err(db_err)?;
            }
            sqlx::query(&format!("PRAGMA user_version = {SCHEMA_VERSION}"))
                .execute(&self.pool)
                .await
                .map_err(db_err)?;
            Ok(())
        }

        async fn ensure_target(&self, target: &TargetConfig) -> Result<()> {
            let parsed = target.to_target()?;
            let now = chrono::Utc::now().to_rfc3339();
            let conditions_json =
                serde_json::to_string(&parsed.conditions).context(SerializeStateSnafu)?;
            sqlx::query("INSERT INTO targets (id, name, url, enabled, conditions_json, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6) ON CONFLICT(id) DO UPDATE SET name = excluded.name, url = excluded.url, enabled = excluded.enabled, conditions_json = excluded.conditions_json, updated_at = excluded.updated_at")
                .bind(&target.id)
                .bind(&target.name)
                .bind(parsed.url)
                .bind(i64::from(target.enabled()))
                .bind(conditions_json)
                .bind(now)
                .execute(&self.pool)
                .await
                .map_err(db_err)?;
            sqlx::query("INSERT OR IGNORE INTO target_state (target_id) VALUES (?1)")
                .bind(&target.id)
                .execute(&self.pool)
                .await
                .map_err(db_err)?;
            Ok(())
        }

        async fn record_success(&self, outcome: &CheckOutcome) -> Result<bool> {
            let checked_at = outcome.checked_at.to_rfc3339();
            let was_matched = sqlx::query_scalar::<_, Option<i64>>(
                "SELECT matched FROM target_state WHERE target_id = ?1",
            )
            .bind(&outcome.target.id)
            .fetch_optional(&self.pool)
            .await
            .map_err(db_err)?
            .flatten()
            .unwrap_or(0)
                != 0;
            let should_alert = outcome.condition_met() && !was_matched;
            let evidence_json =
                serde_json::to_string(&outcome.evidence).context(SerializeStateSnafu)?;
            let condition_results_json =
                serde_json::to_string(&outcome.condition_results).context(SerializeStateSnafu)?;
            let engine = engine_to_str(outcome.engine_used);
            sqlx::query("INSERT INTO checks (target_id, checked_at, matched, engine_used, price_cents, evidence_json, condition_results_json, error) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL)")
                .bind(&outcome.target.id).bind(&checked_at).bind(i64::from(outcome.matched)).bind(engine).bind(outcome.price_cents).bind(&evidence_json).bind(&condition_results_json)
                .execute(&self.pool).await.map_err(db_err)?;
            sqlx::query("UPDATE target_state SET matched = ?2, engine_used = ?3, price_cents = ?4, evidence_json = ?5, condition_results_json = ?6, last_success_at = ?7, last_error_at = NULL, last_error = NULL WHERE target_id = ?1")
                .bind(&outcome.target.id).bind(i64::from(outcome.matched)).bind(engine).bind(outcome.price_cents).bind(evidence_json).bind(condition_results_json).bind(checked_at)
                .execute(&self.pool).await.map_err(db_err)?;
            Ok(should_alert)
        }

        async fn record_error(&self, target_id: &str, error: &str) -> Result<()> {
            let now = chrono::Utc::now().to_rfc3339();
            sqlx::query("INSERT INTO checks (target_id, checked_at, error) VALUES (?1, ?2, ?3)")
                .bind(target_id)
                .bind(&now)
                .bind(error)
                .execute(&self.pool)
                .await
                .map_err(db_err)?;
            sqlx::query(
                "UPDATE target_state SET last_error_at = ?2, last_error = ?3 WHERE target_id = ?1",
            )
            .bind(target_id)
            .bind(now)
            .bind(error)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
            Ok(())
        }

        async fn mark_alert_sent(&self, target_id: &str) -> Result<()> {
            let now = chrono::Utc::now().to_rfc3339();
            sqlx::query("UPDATE target_state SET last_alert_at = ?2 WHERE target_id = ?1")
                .bind(target_id)
                .bind(now)
                .execute(&self.pool)
                .await
                .map_err(db_err)?;
            Ok(())
        }

        async fn statuses(&self) -> Result<Vec<TargetStatus>> {
            sqlx::query(STATUS_SQL)
                .fetch_all(&self.pool)
                .await
                .map_err(db_err)?
                .into_iter()
                .map(|row| {
                    status_from_parts(StatusParts {
                        id: row.try_get("id").map_err(db_err)?,
                        name: row.try_get("name").map_err(db_err)?,
                        url: row.try_get("url").map_err(db_err)?,
                        matched: row.try_get("matched").map_err(db_err)?,
                        engine_used: row.try_get("engine_used").map_err(db_err)?,
                        price_cents: row.try_get("price_cents").map_err(db_err)?,
                        evidence_json: row.try_get("evidence_json").map_err(db_err)?,
                        condition_results_json: row
                            .try_get("condition_results_json")
                            .map_err(db_err)?,
                        last_success_at: row.try_get("last_success_at").map_err(db_err)?,
                        last_error_at: row.try_get("last_error_at").map_err(db_err)?,
                        last_error: row.try_get("last_error").map_err(db_err)?,
                        last_alert_at: row.try_get("last_alert_at").map_err(db_err)?,
                    })
                })
                .collect()
        }
    }

    fn db_err(error: impl std::fmt::Display) -> crate::Error {
        crate::Error::Database {
            message: error.to_string(),
        }
    }
}

#[cfg(all(
    feature = "persistence-seaorm",
    not(feature = "persistence-diesel"),
    not(feature = "persistence-sqlx")
))]
mod active {
    use super::*;
    use sea_orm::{ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement};

    pub const BACKEND_NAME: &str = "seaorm";

    pub async fn connect(path: &str) -> Result<Box<dyn Persistence>> {
        let conn = Database::connect(sqlite_url(path)).await.map_err(db_err)?;
        Ok(Box::new(SeaOrmPersistence { conn }))
    }

    struct SeaOrmPersistence {
        conn: DatabaseConnection,
    }

    #[async_trait]
    impl Persistence for SeaOrmPersistence {
        async fn migrate(&self) -> Result<()> {
            let version = self
                .query_one_i64("PRAGMA user_version", "user_version", vec![])
                .await?
                .unwrap_or(0);
            if version != SCHEMA_VERSION {
                for sql in DROP_TABLES {
                    self.exec(sql, vec![]).await?;
                }
            }
            for sql in CREATE_TABLES {
                self.exec(sql, vec![]).await?;
            }
            self.exec(&format!("PRAGMA user_version = {SCHEMA_VERSION}"), vec![])
                .await
        }

        async fn ensure_target(&self, target: &TargetConfig) -> Result<()> {
            let parsed = target.to_target()?;
            let conditions_json =
                serde_json::to_string(&parsed.conditions).context(SerializeStateSnafu)?;
            self.exec("INSERT INTO targets (id, name, url, enabled, conditions_json, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6) ON CONFLICT(id) DO UPDATE SET name = excluded.name, url = excluded.url, enabled = excluded.enabled, conditions_json = excluded.conditions_json, updated_at = excluded.updated_at",
                vec![target.id.clone().into(), target.name.clone().into(), parsed.url.into(), i64::from(target.enabled()).into(), conditions_json.into(), chrono::Utc::now().to_rfc3339().into()]).await?;
            self.exec(
                "INSERT OR IGNORE INTO target_state (target_id) VALUES (?1)",
                vec![target.id.clone().into()],
            )
            .await
        }

        async fn record_success(&self, outcome: &CheckOutcome) -> Result<bool> {
            let was_matched = self
                .query_one_i64(
                    "SELECT matched FROM target_state WHERE target_id = ?1",
                    "matched",
                    vec![outcome.target.id.clone().into()],
                )
                .await?
                .unwrap_or(0)
                != 0;
            let should_alert = outcome.condition_met() && !was_matched;
            let evidence_json =
                serde_json::to_string(&outcome.evidence).context(SerializeStateSnafu)?;
            let condition_results_json =
                serde_json::to_string(&outcome.condition_results).context(SerializeStateSnafu)?;
            let checked_at = outcome.checked_at.to_rfc3339();
            let engine = engine_to_str(outcome.engine_used).to_string();
            self.exec("INSERT INTO checks (target_id, checked_at, matched, engine_used, price_cents, evidence_json, condition_results_json, error) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL)",
                vec![outcome.target.id.clone().into(), checked_at.clone().into(), i64::from(outcome.matched).into(), engine.clone().into(), outcome.price_cents.into(), evidence_json.clone().into(), condition_results_json.clone().into()]).await?;
            self.exec("UPDATE target_state SET matched = ?2, engine_used = ?3, price_cents = ?4, evidence_json = ?5, condition_results_json = ?6, last_success_at = ?7, last_error_at = NULL, last_error = NULL WHERE target_id = ?1",
                vec![outcome.target.id.clone().into(), i64::from(outcome.matched).into(), engine.into(), outcome.price_cents.into(), evidence_json.into(), condition_results_json.into(), checked_at.into()]).await?;
            Ok(should_alert)
        }

        async fn record_error(&self, target_id: &str, error: &str) -> Result<()> {
            let now = chrono::Utc::now().to_rfc3339();
            self.exec(
                "INSERT INTO checks (target_id, checked_at, error) VALUES (?1, ?2, ?3)",
                vec![target_id.into(), now.clone().into(), error.into()],
            )
            .await?;
            self.exec(
                "UPDATE target_state SET last_error_at = ?2, last_error = ?3 WHERE target_id = ?1",
                vec![target_id.into(), now.into(), error.into()],
            )
            .await
        }

        async fn mark_alert_sent(&self, target_id: &str) -> Result<()> {
            self.exec(
                "UPDATE target_state SET last_alert_at = ?2 WHERE target_id = ?1",
                vec![target_id.into(), chrono::Utc::now().to_rfc3339().into()],
            )
            .await
        }

        async fn statuses(&self) -> Result<Vec<TargetStatus>> {
            self.conn
                .query_all(stmt(STATUS_SQL, vec![]))
                .await
                .map_err(db_err)?
                .into_iter()
                .map(|row| {
                    status_from_parts(StatusParts {
                        id: sea_get(&row, "id")?,
                        name: sea_get(&row, "name")?,
                        url: sea_get(&row, "url")?,
                        matched: sea_get(&row, "matched")?,
                        engine_used: sea_get(&row, "engine_used")?,
                        price_cents: sea_get(&row, "price_cents")?,
                        evidence_json: sea_get(&row, "evidence_json")?,
                        condition_results_json: sea_get(&row, "condition_results_json")?,
                        last_success_at: sea_get(&row, "last_success_at")?,
                        last_error_at: sea_get(&row, "last_error_at")?,
                        last_error: sea_get(&row, "last_error")?,
                        last_alert_at: sea_get(&row, "last_alert_at")?,
                    })
                })
                .collect()
        }
    }

    impl SeaOrmPersistence {
        async fn exec(&self, sql: &str, values: Vec<sea_orm::Value>) -> Result<()> {
            self.conn.execute(stmt(sql, values)).await.map_err(db_err)?;
            Ok(())
        }

        async fn query_one_i64(
            &self,
            sql: &str,
            col: &str,
            values: Vec<sea_orm::Value>,
        ) -> Result<Option<i64>> {
            Ok(self
                .conn
                .query_one(stmt(sql, values))
                .await
                .map_err(db_err)?
                .and_then(|row| row.try_get("", col).ok()))
        }
    }

    fn stmt(sql: &str, values: Vec<sea_orm::Value>) -> Statement {
        Statement::from_sql_and_values(DatabaseBackend::Sqlite, sql, values)
    }

    fn sea_get<T: sea_orm::TryGetable>(row: &sea_orm::QueryResult, column: &str) -> Result<T> {
        row.try_get("", column).map_err(db_err)
    }

    fn sqlite_url(path: &str) -> String {
        if path.starts_with("sqlite:") {
            path.to_string()
        } else {
            format!("sqlite://{path}?mode=rwc")
        }
    }

    fn db_err(error: impl std::fmt::Display) -> crate::Error {
        crate::Error::Database {
            message: error.to_string(),
        }
    }
}

struct StatusParts {
    id: String,
    name: String,
    url: String,
    matched: Option<i64>,
    engine_used: Option<String>,
    price_cents: Option<i64>,
    evidence_json: Option<String>,
    condition_results_json: Option<String>,
    last_success_at: Option<String>,
    last_error_at: Option<String>,
    last_error: Option<String>,
    last_alert_at: Option<String>,
}

fn status_from_parts(parts: StatusParts) -> Result<TargetStatus> {
    Ok(TargetStatus {
        target_id: parts.id,
        name: parts.name,
        url: parts.url,
        matched: parts.matched.map(|value| value != 0),
        engine_used: parts.engine_used.and_then(|value| str_to_engine(&value)),
        price_cents: parts.price_cents,
        evidence: parse_json(parts.evidence_json.as_deref().unwrap_or("[]"))?,
        condition_results: parse_json(parts.condition_results_json.as_deref().unwrap_or("[]"))?,
        last_success_at: parts.last_success_at,
        last_error_at: parts.last_error_at,
        last_error: parts.last_error,
        last_alert_at: parts.last_alert_at,
    })
}

fn engine_to_str(engine: EngineUsed) -> &'static str {
    match engine {
        EngineUsed::Http => "http",
        EngineUsed::BrowserCdp => "browser_cdp",
    }
}

fn str_to_engine(value: &str) -> Option<EngineUsed> {
    match value {
        "http" => Some(EngineUsed::Http),
        "browser_cdp" => Some(EngineUsed::BrowserCdp),
        _ => None,
    }
}

fn parse_json<T: serde::de::DeserializeOwned>(value: &str) -> Result<T> {
    serde_json::from_str(value).context(ParseStateSnafu)
}
