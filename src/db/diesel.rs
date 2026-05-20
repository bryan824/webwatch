use async_trait::async_trait;
use snafu::ResultExt;

use crate::{
    config::TargetConfig,
    config::{CheckOutcome, TargetStatus},
    error::{Result, SerializeStateSnafu},
};

use super::{
    engine_to_str, status_from_parts, Persistence, StatusParts, CREATE_TABLES, DROP_TABLES,
    SCHEMA_VERSION, STATUS_SQL,
};

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
            sql_query(
                "UPDATE target_state SET last_error_at = ?2, last_error = ?3 WHERE target_id = ?1",
            )
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
