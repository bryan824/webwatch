use std::{env, fs, time::Duration};

use serde::{Deserialize, Serialize};
use snafu::{ensure, ResultExt};

use crate::{
    error::{EmptyTargetsSnafu, ParseConfigSnafu, ReadConfigSnafu, Result},
    models::{ConditionKind, Target},
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
    pub targets: Vec<TargetConfig>,
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TargetConfig {
    pub id: String,
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub interval_secs: Option<u64>,
    #[serde(default)]
    pub conditions: Vec<ConditionConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConditionConfig {
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
        for target in &self.targets {
            Target::from_config(target)?;
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

impl TargetConfig {
    pub fn enabled(&self) -> bool {
        self.enabled.unwrap_or(true)
    }

    pub fn interval_secs(&self, config: &AppConfig) -> u64 {
        self.interval_secs
            .unwrap_or(config.scheduler.default_interval_secs)
    }

    pub fn to_target(&self) -> Result<Target> {
        Target::from_config(self)
    }
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
