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
