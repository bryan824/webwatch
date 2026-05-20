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
    error::{ParseStateSnafu, Result},
    models::{CheckOutcome, EngineUsed, TargetStatus},
};

#[cfg(all(
    feature = "persistence-diesel",
    not(feature = "persistence-sqlx"),
    not(feature = "persistence-seaorm")
))]
mod diesel;
#[cfg(all(
    feature = "persistence-seaorm",
    not(feature = "persistence-diesel"),
    not(feature = "persistence-sqlx")
))]
mod seaorm;
#[cfg(all(
    feature = "persistence-sqlx",
    not(feature = "persistence-diesel"),
    not(feature = "persistence-seaorm")
))]
mod sqlx;

#[cfg(all(
    feature = "persistence-diesel",
    not(feature = "persistence-sqlx"),
    not(feature = "persistence-seaorm")
))]
use self::diesel as backend;
#[cfg(all(
    feature = "persistence-seaorm",
    not(feature = "persistence-diesel"),
    not(feature = "persistence-sqlx")
))]
use self::seaorm as backend;
#[cfg(all(
    feature = "persistence-sqlx",
    not(feature = "persistence-diesel"),
    not(feature = "persistence-seaorm")
))]
use self::sqlx as backend;

pub(crate) const SCHEMA_VERSION: i64 = 1;
pub(crate) const DROP_TABLES: [&str; 3] = [
    "DROP TABLE IF EXISTS checks",
    "DROP TABLE IF EXISTS target_state",
    "DROP TABLE IF EXISTS targets",
];
pub(crate) const CREATE_TABLES: [&str; 3] = [
    "CREATE TABLE IF NOT EXISTS targets (id TEXT PRIMARY KEY, name TEXT NOT NULL, url TEXT NOT NULL, enabled INTEGER NOT NULL DEFAULT 1, conditions_json TEXT NOT NULL, updated_at TEXT NOT NULL)",
    "CREATE TABLE IF NOT EXISTS target_state (target_id TEXT PRIMARY KEY REFERENCES targets(id) ON DELETE CASCADE, matched INTEGER, engine_used TEXT, price_cents INTEGER, evidence_json TEXT NOT NULL DEFAULT '[]', condition_results_json TEXT NOT NULL DEFAULT '[]', last_success_at TEXT, last_error_at TEXT, last_error TEXT, last_alert_at TEXT)",
    "CREATE TABLE IF NOT EXISTS checks (id INTEGER PRIMARY KEY AUTOINCREMENT, target_id TEXT NOT NULL REFERENCES targets(id) ON DELETE CASCADE, checked_at TEXT NOT NULL, matched INTEGER, engine_used TEXT, price_cents INTEGER, evidence_json TEXT NOT NULL DEFAULT '[]', condition_results_json TEXT NOT NULL DEFAULT '[]', error TEXT)",
];
pub(crate) const STATUS_SQL: &str = "SELECT t.id, t.name, t.url, s.matched, s.engine_used, s.price_cents, s.evidence_json, s.condition_results_json, s.last_success_at, s.last_error_at, s.last_error, s.last_alert_at FROM targets t LEFT JOIN target_state s ON s.target_id = t.id ORDER BY t.id";

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
    backend::connect(path).await
}

pub fn backend_name() -> &'static str {
    backend::BACKEND_NAME
}

pub(crate) struct StatusParts {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) url: String,
    pub(crate) matched: Option<i64>,
    pub(crate) engine_used: Option<String>,
    pub(crate) price_cents: Option<i64>,
    pub(crate) evidence_json: Option<String>,
    pub(crate) condition_results_json: Option<String>,
    pub(crate) last_success_at: Option<String>,
    pub(crate) last_error_at: Option<String>,
    pub(crate) last_error: Option<String>,
    pub(crate) last_alert_at: Option<String>,
}

pub(crate) fn status_from_parts(parts: StatusParts) -> Result<TargetStatus> {
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

pub(crate) fn engine_to_str(engine: EngineUsed) -> &'static str {
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
