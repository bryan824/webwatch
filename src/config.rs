use std::{env, fs, time::Duration};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use snafu::{ensure, ResultExt};
use url::Url;

use crate::error::{
    EmptyConditionsSnafu, EmptyTargetsSnafu, ParseConfigSnafu, ParseTargetUrlSnafu,
    ReadConfigSnafu, Result,
};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig {
    #[serde(default = "default_sqlite_path")]
    pub sqlite_path: String,
    #[serde(default = "default_user_agent")]
    pub user_agent: String,
    #[serde(default)]
    pub discord_webhook_url: Option<String>,
    #[serde(default)]
    pub api_token: Option<String>,
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub scheduler: SchedulerConfig,
    #[serde(default)]
    pub browser: BrowserConfig,
    #[serde(default)]
    pub targets: Vec<Target>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    #[serde(default = "default_bind")]
    pub bind: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SchedulerConfig {
    #[serde(default = "default_interval_secs")]
    pub default_interval_secs: u64,
    #[serde(default = "default_jitter_secs")]
    pub jitter_secs: u64,
    #[serde(default = "default_http_timeout_secs")]
    pub http_timeout_secs: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct BrowserConfig {
    #[serde(default)]
    pub cdp_url: Option<String>,
    #[serde(default = "default_browser_wait_ms")]
    pub wait_ms: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct Target {
    pub id: String,
    pub name: String,
    #[serde(deserialize_with = "deserialize_url")]
    pub url: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub interval_secs: Option<u64>,
    #[serde(default)]
    pub conditions: Vec<Condition>,
}

pub type TargetConfig = Target;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct Condition {
    #[serde(default)]
    pub id: Option<String>,
    pub kind: ConditionKind,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub selector: Option<String>,
    #[serde(default)]
    pub threshold_cents: Option<i64>,
    #[serde(default)]
    pub price_selector: Option<String>,
}

pub type ConditionConfig = Condition;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConditionKind {
    TextAppears,
    TextDisappears,
    SelectorExists,
    SelectorMissing,
    SelectorTextContains,
    SelectorTextNotContains,
    PriceBelow,
    PriceAbove,
    PriceChanged,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EngineUsed {
    Http,
    BrowserCdp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConditionResult {
    pub condition_id: String,
    pub kind: ConditionKind,
    pub matched: bool,
    pub evidence: Vec<String>,
    pub observed_price_cents: Option<i64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckOutcome {
    pub target: Target,
    pub engine_used: EngineUsed,
    pub matched: bool,
    pub checked_at: DateTime<Utc>,
    pub price_cents: Option<i64>,
    pub evidence: Vec<String>,
    pub condition_results: Vec<ConditionResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetStatus {
    pub target_id: String,
    pub name: String,
    pub url: String,
    pub matched: Option<bool>,
    pub engine_used: Option<EngineUsed>,
    pub price_cents: Option<i64>,
    pub evidence: Vec<String>,
    pub condition_results: Vec<ConditionResult>,
    pub last_success_at: Option<String>,
    pub last_error_at: Option<String>,
    pub last_error: Option<String>,
    pub last_alert_at: Option<String>,
}

impl AppConfig {
    pub fn load(path: &str) -> Result<Self> {
        let raw = fs::read_to_string(path).context(ReadConfigSnafu {
            path: path.to_string(),
        })?;
        let config: AppConfig = toml::from_str(&raw).context(ParseConfigSnafu {
            path: path.to_string(),
        })?;

        config.resolve_env_and_validate()
    }

    pub fn resolve_env_and_validate(mut self) -> Result<Self> {
        if self.discord_webhook_url.is_none() {
            self.discord_webhook_url = env::var("DISCORD_WEBHOOK_URL").ok();
        }
        if self.api_token.is_none() {
            self.api_token = env::var("WEBWATCH_API_TOKEN").ok();
        }

        ensure!(!self.targets.is_empty(), EmptyTargetsSnafu);
        for target in &mut self.targets {
            target.validate_and_resolve()?;
        }

        Ok(self)
    }

    pub fn http_timeout(&self) -> Duration {
        Duration::from_secs(self.scheduler.http_timeout_secs)
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: default_bind(),
        }
    }
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            default_interval_secs: default_interval_secs(),
            jitter_secs: default_jitter_secs(),
            http_timeout_secs: default_http_timeout_secs(),
        }
    }
}

impl Target {
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn interval_secs(&self, config: &AppConfig) -> u64 {
        self.interval_secs
            .unwrap_or(config.scheduler.default_interval_secs)
    }

    pub fn to_target(&self) -> Result<Target> {
        let mut target = self.clone();
        target.validate_and_resolve()?;
        Ok(target)
    }

    fn validate_and_resolve(&mut self) -> Result<()> {
        Url::parse(&self.url).context(ParseTargetUrlSnafu {
            target_id: self.id.clone(),
        })?;
        ensure!(
            !self.conditions.is_empty(),
            EmptyConditionsSnafu {
                target_id: self.id.clone()
            }
        );
        for (index, condition) in self.conditions.iter_mut().enumerate() {
            if condition.id.is_none() {
                condition.id = Some(format!("condition-{}", index + 1));
            }
        }
        Ok(())
    }
}

impl CheckOutcome {
    pub fn condition_met(&self) -> bool {
        self.matched
    }
}

fn deserialize_url<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    Url::parse(&value).map_err(serde::de::Error::custom)?;
    Ok(value)
}

fn default_true() -> bool {
    true
}

fn default_sqlite_path() -> String {
    "webwatch.sqlite3".to_string()
}

fn default_user_agent() -> String {
    "webwatch/0.1 (+https://example.invalid; low-frequency page monitor)".to_string()
}

fn default_bind() -> String {
    "127.0.0.1:3000".to_string()
}

fn default_interval_secs() -> u64 {
    300
}

fn default_jitter_secs() -> u64 {
    30
}

fn default_http_timeout_secs() -> u64 {
    20
}

fn default_browser_wait_ms() -> u64 {
    5_000
}

#[cfg(test)]
mod tests {
    use super::{AppConfig, ConditionKind};

    #[test]
    fn builds_generic_target_from_url_and_conditions() {
        let raw = r#"
            [[targets]]
            id = "campfire"
            name = "Campfire Mug"
            url = "https://example.com/product"

            [[targets.conditions]]
            kind = "text_appears"
            value = "Add to cart"
        "#;

        let config = toml::from_str::<AppConfig>(raw)
            .expect("parse config")
            .resolve_env_and_validate()
            .expect("valid target");

        let target = &config.targets[0];
        assert_eq!(target.url, "https://example.com/product");
        assert_eq!(target.conditions[0].id.as_deref(), Some("condition-1"));
        assert_eq!(target.conditions[0].kind, ConditionKind::TextAppears);
    }
}
