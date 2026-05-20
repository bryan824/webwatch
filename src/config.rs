use std::{env, fs, time::Duration};

use chrono::{DateTime, Utc};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Condition {
    pub id: Option<String>,
    pub kind: ConditionKind,
    pub negate: bool,
    pub value: Option<String>,
    pub selector: Option<String>,
    pub threshold_cents: Option<i64>,
    pub price_selector: Option<String>,
}

pub type ConditionConfig = Condition;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConditionKind {
    Text,
    Selector,
    SelectorText,
    Price,
    PriceObserved,
}

#[derive(Debug, Deserialize)]
struct ConditionRaw {
    #[serde(default)]
    id: Option<String>,
    kind: String,
    #[serde(default)]
    value: Option<String>,
    #[serde(default)]
    selector: Option<String>,
    #[serde(default)]
    threshold_cents: Option<i64>,
    #[serde(default)]
    price_selector: Option<String>,
}

impl<'de> Deserialize<'de> for Condition {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = ConditionRaw::deserialize(deserializer)?;
        let (kind, negate) = kind_from_wire(&raw.kind).map_err(serde::de::Error::custom)?;
        Ok(Self {
            id: raw.id,
            kind,
            negate,
            value: raw.value,
            selector: raw.selector,
            threshold_cents: raw.threshold_cents,
            price_selector: raw.price_selector,
        })
    }
}

impl Serialize for Condition {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("Condition", 7)?;
        state.serialize_field("id", &self.id)?;
        state.serialize_field("kind", wire_from_kind(self.kind, self.negate))?;
        state.serialize_field("value", &self.value)?;
        state.serialize_field("selector", &self.selector)?;
        state.serialize_field("threshold_cents", &self.threshold_cents)?;
        state.serialize_field("price_selector", &self.price_selector)?;
        state.end()
    }
}

fn kind_from_wire(value: &str) -> std::result::Result<(ConditionKind, bool), String> {
    match value {
        "text_appears" => Ok((ConditionKind::Text, false)),
        "text_disappears" => Ok((ConditionKind::Text, true)),
        "selector_exists" => Ok((ConditionKind::Selector, false)),
        "selector_missing" => Ok((ConditionKind::Selector, true)),
        "selector_text_contains" => Ok((ConditionKind::SelectorText, false)),
        "selector_text_not_contains" => Ok((ConditionKind::SelectorText, true)),
        "price_below" => Ok((ConditionKind::Price, false)),
        "price_above" => Ok((ConditionKind::Price, true)),
        "price_changed" => Ok((ConditionKind::PriceObserved, false)),
        other => Err(format!("unknown condition kind '{other}'")),
    }
}

fn wire_from_kind(kind: ConditionKind, negate: bool) -> &'static str {
    match (kind, negate) {
        (ConditionKind::Text, false) => "text_appears",
        (ConditionKind::Text, true) => "text_disappears",
        (ConditionKind::Selector, false) => "selector_exists",
        (ConditionKind::Selector, true) => "selector_missing",
        (ConditionKind::SelectorText, false) => "selector_text_contains",
        (ConditionKind::SelectorText, true) => "selector_text_not_contains",
        (ConditionKind::Price, false) => "price_below",
        (ConditionKind::Price, true) => "price_above",
        (ConditionKind::PriceObserved, false) => "price_changed",
        (ConditionKind::PriceObserved, true) => "price_changed",
    }
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
    use super::{AppConfig, Condition, ConditionKind};

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
        assert_eq!(target.conditions[0].kind, ConditionKind::Text);
        assert!(!target.conditions[0].negate);
    }

    #[test]
    fn deserializes_legacy_condition_strings() {
        let cases = [
            ("text_appears", ConditionKind::Text, false),
            ("text_disappears", ConditionKind::Text, true),
            ("selector_exists", ConditionKind::Selector, false),
            ("selector_missing", ConditionKind::Selector, true),
            ("selector_text_contains", ConditionKind::SelectorText, false),
            (
                "selector_text_not_contains",
                ConditionKind::SelectorText,
                true,
            ),
            ("price_below", ConditionKind::Price, false),
            ("price_above", ConditionKind::Price, true),
            ("price_changed", ConditionKind::PriceObserved, false),
        ];

        for (wire, kind, negate) in cases {
            let condition = toml::from_str::<Condition>(&format!("kind = \"{wire}\""))
                .expect("parse condition");
            assert_eq!(condition.kind, kind);
            assert_eq!(condition.negate, negate);
        }
    }

    #[test]
    fn serializes_back_to_legacy_strings() {
        let condition = Condition {
            id: Some("gone".to_string()),
            kind: ConditionKind::Text,
            negate: true,
            value: Some("Add to cart".to_string()),
            selector: None,
            threshold_cents: None,
            price_selector: None,
        };

        let encoded = toml::to_string(&condition).expect("serialize condition");
        assert!(encoded.contains("kind = \"text_disappears\""));
    }
}
