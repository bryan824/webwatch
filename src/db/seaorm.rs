use async_trait::async_trait;
use snafu::ResultExt;

use crate::{
    config::TargetConfig,
    error::{Result, SerializeStateSnafu},
    models::{CheckOutcome, TargetStatus},
};

use super::{
    engine_to_str, status_from_parts, Persistence, StatusParts, CREATE_TABLES, DROP_TABLES,
    SCHEMA_VERSION, STATUS_SQL,
};

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
