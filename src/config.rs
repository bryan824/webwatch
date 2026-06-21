use std::{
    collections::HashSet,
    env, fs,
    path::{Path, PathBuf},
    time::Duration,
};

use chrono::{DateTime, Utc};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use snafu::{ensure, ResultExt};
use url::Url;

use crate::error::{
    EmptyConditionsSnafu, InvalidRenderConfigSnafu, MissingConditionFieldSnafu, ParseConfigSnafu,
    ParseTargetUrlSnafu, ReadConfigSnafu, ReadTargetsSnafu, Result,
};

#[derive(Debug, Clone, Serialize)]
pub struct AppConfig {
    pub sqlite_path: String,
    pub user_agent: String,
    pub discord_webhook_url: Option<String>,
    pub targets_path: Option<String>,
    pub server: ServerConfig,
    pub scheduler: SchedulerConfig,
    pub browser: BrowserConfig,
    pub renderer: RendererConfig,
}

#[derive(Debug, Deserialize)]
struct AppConfigRaw {
    #[serde(default = "default_sqlite_path")]
    sqlite_path: String,
    #[serde(default = "default_user_agent")]
    user_agent: String,
    #[serde(default)]
    discord_webhook_url: Option<String>,
    #[serde(default = "default_targets_path")]
    targets_path: Option<String>,
    #[serde(default)]
    server: ServerConfig,
    #[serde(default)]
    scheduler: SchedulerConfig,
    #[serde(default)]
    browser: BrowserConfig,
    #[serde(default)]
    renderer: Option<RendererConfig>,
}

impl<'de> Deserialize<'de> for AppConfig {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = AppConfigRaw::deserialize(deserializer)?;
        let renderer = raw
            .renderer
            .unwrap_or_else(|| RendererConfig::from_legacy_browser(&raw.browser));
        Ok(Self {
            sqlite_path: raw.sqlite_path,
            user_agent: raw.user_agent,
            discord_webhook_url: raw.discord_webhook_url,
            targets_path: raw.targets_path,
            server: raw.server,
            scheduler: raw.scheduler,
            browser: raw.browser,
            renderer,
        })
    }
}

impl AppConfig {
    /// Directory holding the last rendered snapshot (HTML + screenshot) for a
    /// target, kept next to the SQLite database so status queries stay lean.
    pub fn snapshot_dir(&self, target_id: &str) -> PathBuf {
        Path::new(&self.sqlite_path)
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
            .join("snapshots")
            .join(target_id)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TargetsFile {
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

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct BrowserConfig {
    #[serde(default)]
    pub cdp_url: Option<String>,
    #[serde(default = "default_browser_wait_ms")]
    pub wait_ms: u64,
}

impl Default for BrowserConfig {
    fn default() -> Self {
        Self {
            cdp_url: None,
            wait_ms: default_browser_wait_ms(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct RendererConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub backend: RendererBackend,
    #[serde(default)]
    pub endpoint: Option<String>,
    #[serde(default = "default_renderer_max_concurrency")]
    pub max_concurrency: usize,
    #[serde(default = "default_renderer_navigation_timeout_ms")]
    pub navigation_timeout_ms: u64,
    #[serde(default = "default_renderer_operation_timeout_ms")]
    pub operation_timeout_ms: u64,
    #[serde(default = "default_renderer_settle_ms")]
    pub settle_ms: u64,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RendererBackend {
    #[default]
    #[serde(rename = "cloakbrowser")]
    CloakBrowser,
    Cdp,
}

impl Default for RendererConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            backend: RendererBackend::default(),
            endpoint: None,
            max_concurrency: default_renderer_max_concurrency(),
            navigation_timeout_ms: default_renderer_navigation_timeout_ms(),
            operation_timeout_ms: default_renderer_operation_timeout_ms(),
            settle_ms: default_renderer_settle_ms(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct RenderPlan {
    #[serde(default)]
    pub policy: RenderPolicy,
    #[serde(default)]
    pub fingerprint_seed: Option<String>,
    #[serde(default)]
    pub wait_ms: Option<u64>,
    #[serde(default)]
    pub scenario_match: ScenarioMatch,
    #[serde(default)]
    pub steps: Vec<RenderStep>,
    #[serde(default)]
    pub scenarios: Vec<RenderScenario>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RenderPolicy {
    HttpOnly,
    #[default]
    Auto,
    RenderFirst,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScenarioMatch {
    #[default]
    Any,
    All,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct RenderStep {
    pub op: RenderOperation,
    #[serde(default)]
    pub selector: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub option_text: Option<String>,
    #[serde(default)]
    pub option_value: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    #[serde(default)]
    pub settle_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RenderOperation {
    WaitFor,
    WaitForText,
    Click,
    Select,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct RenderScenario {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub steps: Vec<RenderStep>,
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
    pub render: RenderPlan,
    #[serde(default)]
    pub conditions: Vec<Condition>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Condition {
    pub id: Option<String>,
    pub rule: ConditionRule,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConditionRule {
    Text {
        value: String,
        negate: bool,
    },
    Selector {
        selector: String,
        negate: bool,
    },
    SelectorText {
        selector: String,
        value: String,
        negate: bool,
    },
    Price {
        threshold_cents: i64,
        selector: Option<String>,
        price_selector: Option<String>,
        negate: bool,
    },
    PriceObserved {
        selector: Option<String>,
        price_selector: Option<String>,
        negate: bool,
    },
    Redirect {
        negate: bool,
    },
    Invalid {
        kind: ConditionKind,
        negate: bool,
        missing_field: &'static str,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConditionKind {
    Text,
    Selector,
    SelectorText,
    Price,
    PriceObserved,
    Redirect,
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
            rule: rule_from_raw(
                kind,
                negate,
                raw.value,
                raw.selector,
                raw.threshold_cents,
                raw.price_selector,
            ),
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
        state.serialize_field("kind", wire_from_kind(self.kind(), self.negate()))?;
        state.serialize_field("value", &self.value())?;
        state.serialize_field("selector", &self.selector())?;
        state.serialize_field("threshold_cents", &self.threshold_cents())?;
        state.serialize_field("price_selector", &self.price_selector())?;
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
        "url_redirects" => Ok((ConditionKind::Redirect, false)),
        "url_unchanged" => Ok((ConditionKind::Redirect, true)),
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
        (ConditionKind::Redirect, false) => "url_redirects",
        (ConditionKind::Redirect, true) => "url_unchanged",
    }
}

fn rule_from_raw(
    kind: ConditionKind,
    negate: bool,
    value: Option<String>,
    selector: Option<String>,
    threshold_cents: Option<i64>,
    price_selector: Option<String>,
) -> ConditionRule {
    match kind {
        ConditionKind::Text => value
            .map(|value| ConditionRule::Text { value, negate })
            .unwrap_or(ConditionRule::Invalid {
                kind,
                negate,
                missing_field: "value",
            }),
        ConditionKind::Selector => selector
            .map(|selector| ConditionRule::Selector { selector, negate })
            .unwrap_or(ConditionRule::Invalid {
                kind,
                negate,
                missing_field: "selector",
            }),
        ConditionKind::SelectorText => match (selector, value) {
            (Some(selector), Some(value)) => ConditionRule::SelectorText {
                selector,
                value,
                negate,
            },
            (None, _) => ConditionRule::Invalid {
                kind,
                negate,
                missing_field: "selector",
            },
            (_, None) => ConditionRule::Invalid {
                kind,
                negate,
                missing_field: "value",
            },
        },
        ConditionKind::Price => threshold_cents
            .map(|threshold_cents| ConditionRule::Price {
                threshold_cents,
                selector,
                price_selector,
                negate,
            })
            .unwrap_or(ConditionRule::Invalid {
                kind,
                negate,
                missing_field: "threshold_cents",
            }),
        ConditionKind::PriceObserved => ConditionRule::PriceObserved {
            selector,
            price_selector,
            negate,
        },
        ConditionKind::Redirect => ConditionRule::Redirect { negate },
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
    #[serde(default)]
    pub scenario_id: Option<String>,
    #[serde(default)]
    pub scenario_label: Option<String>,
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
    pub enabled: bool,
    pub render: RenderPlan,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckRun {
    pub checked_at: String,
    pub matched: Option<bool>,
    pub engine_used: Option<EngineUsed>,
    pub price_cents: Option<i64>,
    pub evidence: Vec<String>,
    pub condition_results: Vec<ConditionResult>,
    pub error: Option<String>,
}

impl AppConfig {
    pub fn load(path: &str) -> Result<(Self, TargetsFile)> {
        let raw = fs::read_to_string(path).context(ReadConfigSnafu {
            path: path.to_string(),
        })?;
        let config: AppConfig = toml::from_str(&raw).context(ParseConfigSnafu {
            path: path.to_string(),
        })?;
        let mut config = config.resolve_env()?;
        config.validate()?;
        let targets_path = config.resolved_targets_path(path);
        config.targets_path = Some(targets_path.display().to_string());
        let targets = TargetsFile::load(&targets_path)?;

        Ok((config, targets))
    }

    pub fn resolve_env(mut self) -> Result<Self> {
        if self.discord_webhook_url.is_none() {
            self.discord_webhook_url = env::var("DISCORD_WEBHOOK_URL").ok();
        }
        if let Ok(path) = env::var("WEBWATCH_TARGETS") {
            self.targets_path = Some(path);
        }

        Ok(self)
    }

    pub fn resolved_targets_path(&self, config_path: &str) -> PathBuf {
        let default_path = default_targets_path().unwrap_or_else(|| "targets.toml".to_string());
        let targets_path = self
            .targets_path
            .as_deref()
            .unwrap_or(default_path.as_str());
        let path = Path::new(targets_path);
        if path.is_absolute() {
            return path.to_path_buf();
        }
        Path::new(config_path)
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(path)
    }

    pub fn http_timeout(&self) -> Duration {
        Duration::from_secs(self.scheduler.http_timeout_secs)
    }

    pub fn validate(&self) -> Result<()> {
        self.renderer.validate()
    }
}

impl TargetsFile {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self {
                targets: Vec::new(),
            });
        }
        let path_string = path.display().to_string();
        let raw = fs::read_to_string(path).context(ReadTargetsSnafu {
            path: path_string.clone(),
        })?;
        let targets: TargetsFile =
            toml::from_str(&raw).context(ParseConfigSnafu { path: path_string })?;
        targets.resolve_and_validate()
    }

    pub fn resolve_and_validate(mut self) -> Result<Self> {
        for target in &mut self.targets {
            target.validate_and_resolve()?;
        }
        Ok(self)
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

impl RendererConfig {
    fn from_legacy_browser(browser: &BrowserConfig) -> Self {
        match browser.cdp_url.clone() {
            Some(endpoint) => Self {
                enabled: true,
                backend: RendererBackend::Cdp,
                endpoint: Some(endpoint),
                max_concurrency: default_renderer_max_concurrency(),
                navigation_timeout_ms: default_renderer_navigation_timeout_ms(),
                operation_timeout_ms: default_renderer_operation_timeout_ms(),
                settle_ms: browser.wait_ms,
            },
            None => Self::default(),
        }
    }

    fn validate(&self) -> Result<()> {
        if self.enabled {
            self.require(
                self.endpoint
                    .as_deref()
                    .is_some_and(|value| !value.trim().is_empty()),
                "enabled renderer requires endpoint",
            )?;
        }
        self.require(
            self.max_concurrency > 0,
            "max_concurrency must be greater than 0",
        )?;
        self.require(
            self.navigation_timeout_ms > 0,
            "navigation_timeout_ms must be greater than 0",
        )?;
        self.require(
            self.operation_timeout_ms > 0,
            "operation_timeout_ms must be greater than 0",
        )?;
        Ok(())
    }

    fn require(&self, valid: bool, message: &'static str) -> Result<()> {
        ensure!(
            valid,
            InvalidRenderConfigSnafu {
                target_id: "global",
                message
            }
        );
        Ok(())
    }
}

impl RenderPlan {
    fn validate(&self, target_id: &str) -> Result<()> {
        if let Some(seed) = &self.fingerprint_seed {
            self.require(
                target_id,
                !seed.trim().is_empty(),
                "fingerprint_seed must not be empty",
            )?;
        }
        if let Some(wait_ms) = self.wait_ms {
            self.require(target_id, wait_ms > 0, "wait_ms must be greater than 0")?;
        }
        self.require(
            target_id,
            self.scenarios.len() <= default_renderer_max_scenarios(),
            "render scenarios exceed the maximum of 10",
        )?;
        for step in &self.steps {
            step.validate(target_id)?;
        }
        let mut scenario_ids = HashSet::new();
        for scenario in &self.scenarios {
            self.require(
                target_id,
                !scenario.id.trim().is_empty(),
                "scenario id must not be empty",
            )?;
            self.require(
                target_id,
                scenario_ids.insert(scenario.id.as_str()),
                "scenario id must be unique",
            )?;
            self.require(
                target_id,
                !scenario.label.trim().is_empty(),
                "scenario label must not be empty",
            )?;
            for step in &scenario.steps {
                step.validate(target_id)?;
            }
        }
        Ok(())
    }

    fn require(&self, target_id: &str, valid: bool, message: &'static str) -> Result<()> {
        ensure!(
            valid,
            InvalidRenderConfigSnafu {
                target_id: target_id.to_string(),
                message
            }
        );
        Ok(())
    }
}

impl RenderStep {
    fn validate(&self, target_id: &str) -> Result<()> {
        self.validate_required_fields(target_id)?;
        self.validate_safe_text(target_id)?;
        self.validate_positive_timing(target_id)?;
        Ok(())
    }

    fn validate_required_fields(&self, target_id: &str) -> Result<()> {
        match self.op {
            RenderOperation::WaitFor => {
                self.require(
                    target_id,
                    self.selector
                        .as_deref()
                        .is_some_and(|value| !value.trim().is_empty()),
                    "wait_for render step requires selector",
                )?;
            }
            RenderOperation::WaitForText => {
                self.require(
                    target_id,
                    self.text
                        .as_deref()
                        .is_some_and(|value| !value.trim().is_empty()),
                    "wait_for_text render step requires text",
                )?;
            }
            RenderOperation::Click => {
                self.require(
                    target_id,
                    self.selector
                        .as_deref()
                        .is_some_and(|value| !value.trim().is_empty()),
                    "click render step requires selector",
                )?;
            }
            RenderOperation::Select => {
                self.require(
                    target_id,
                    self.selector
                        .as_deref()
                        .is_some_and(|value| !value.trim().is_empty()),
                    "select render step requires selector",
                )?;
                let options = usize::from(self.option_text.is_some())
                    + usize::from(self.option_value.is_some());
                self.require(
                    target_id,
                    options == 1,
                    "select render step requires exactly one of option_text or option_value",
                )?;
            }
        }
        Ok(())
    }

    fn validate_safe_text(&self, target_id: &str) -> Result<()> {
        for text in [
            &self.text,
            &self.option_text,
            &self.option_value,
            &self.value,
        ]
        .into_iter()
        .flatten()
        {
            let normalized = text.to_ascii_lowercase();
            let dangerous = [
                "add to cart",
                "checkout",
                "place order",
                "buy now",
                "payment",
                "submit order",
            ]
            .into_iter()
            .any(|phrase| normalized.contains(phrase));
            ensure!(
                !dangerous,
                InvalidRenderConfigSnafu {
                    target_id: target_id.to_string(),
                    message: "render step targets a dangerous action",
                }
            );
        }
        Ok(())
    }

    fn validate_positive_timing(&self, target_id: &str) -> Result<()> {
        if let Some(timeout_ms) = self.timeout_ms {
            self.require(
                target_id,
                timeout_ms > 0,
                "render step timeout_ms must be greater than 0",
            )?;
        }
        if let Some(settle_ms) = self.settle_ms {
            self.require(
                target_id,
                settle_ms > 0,
                "render step settle_ms must be greater than 0",
            )?;
        }
        Ok(())
    }

    fn require(&self, target_id: &str, valid: bool, message: &'static str) -> Result<()> {
        ensure!(
            valid,
            InvalidRenderConfigSnafu {
                target_id: target_id.to_string(),
                message
            }
        );
        Ok(())
    }
}

impl Condition {
    pub fn kind(&self) -> ConditionKind {
        match &self.rule {
            ConditionRule::Text { .. } => ConditionKind::Text,
            ConditionRule::Selector { .. } => ConditionKind::Selector,
            ConditionRule::SelectorText { .. } => ConditionKind::SelectorText,
            ConditionRule::Price { .. } => ConditionKind::Price,
            ConditionRule::PriceObserved { .. } => ConditionKind::PriceObserved,
            ConditionRule::Redirect { .. } => ConditionKind::Redirect,
            ConditionRule::Invalid { kind, .. } => *kind,
        }
    }

    pub fn negate(&self) -> bool {
        match &self.rule {
            ConditionRule::Text { negate, .. }
            | ConditionRule::Selector { negate, .. }
            | ConditionRule::SelectorText { negate, .. }
            | ConditionRule::Price { negate, .. }
            | ConditionRule::PriceObserved { negate, .. }
            | ConditionRule::Redirect { negate, .. }
            | ConditionRule::Invalid { negate, .. } => *negate,
        }
    }

    pub fn value(&self) -> Option<&str> {
        match &self.rule {
            ConditionRule::Text { value, .. } | ConditionRule::SelectorText { value, .. } => {
                Some(value)
            }
            _ => None,
        }
    }

    pub fn selector(&self) -> Option<&str> {
        match &self.rule {
            ConditionRule::Selector { selector, .. }
            | ConditionRule::SelectorText { selector, .. } => Some(selector),
            ConditionRule::Price { selector, .. }
            | ConditionRule::PriceObserved { selector, .. } => selector.as_deref(),
            _ => None,
        }
    }

    pub fn threshold_cents(&self) -> Option<i64> {
        match &self.rule {
            ConditionRule::Price {
                threshold_cents, ..
            } => Some(*threshold_cents),
            _ => None,
        }
    }

    pub fn price_selector(&self) -> Option<&str> {
        match &self.rule {
            ConditionRule::Price { price_selector, .. }
            | ConditionRule::PriceObserved { price_selector, .. } => price_selector.as_deref(),
            _ => None,
        }
    }

    fn validate_required_fields(&self) -> Result<()> {
        if let ConditionRule::Invalid { missing_field, .. } = self.rule {
            self.require_field(false, missing_field)?;
        }
        Ok(())
    }

    fn require_field(&self, present: bool, field: &'static str) -> Result<()> {
        ensure!(
            present,
            MissingConditionFieldSnafu {
                condition_id: self.id.clone().unwrap_or_else(|| "condition".to_string()),
                field,
            }
        );
        Ok(())
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

    pub fn validated(mut self) -> Result<Self> {
        self.validate_and_resolve()?;
        Ok(self)
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
        self.render.validate(&self.id)?;
        for (index, condition) in self.conditions.iter_mut().enumerate() {
            if condition.id.is_none() {
                condition.id = Some(format!("condition-{}", index + 1));
            }
            condition.validate_required_fields()?;
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

fn default_targets_path() -> Option<String> {
    Some("targets.toml".to_string())
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

fn default_renderer_max_concurrency() -> usize {
    1
}

fn default_renderer_navigation_timeout_ms() -> u64 {
    30_000
}

fn default_renderer_operation_timeout_ms() -> u64 {
    10_000
}

fn default_renderer_settle_ms() -> u64 {
    750
}

fn default_renderer_max_scenarios() -> usize {
    10
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{
        AppConfig, Condition, ConditionKind, ConditionRule, RenderOperation, RenderPolicy,
        RendererBackend, TargetsFile,
    };

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

        let targets = toml::from_str::<TargetsFile>(raw)
            .expect("parse targets")
            .resolve_and_validate()
            .expect("valid target");

        let target = &targets.targets[0];
        assert_eq!(target.url, "https://example.com/product");
        assert_eq!(target.conditions[0].id.as_deref(), Some("condition-1"));
        assert_eq!(target.conditions[0].kind(), ConditionKind::Text);
        assert!(!target.conditions[0].negate());
    }

    #[test]
    fn loads_split_files() {
        let dir = tempfile::tempdir().expect("tempdir");
        fs::write(
            dir.path().join("config.toml"),
            r#"
                sqlite_path = "webwatch.sqlite3"
                targets_path = "targets.toml"
            "#,
        )
        .expect("write config");
        fs::write(
            dir.path().join("targets.toml"),
            r#"
                [[targets]]
                id = "campfire"
                name = "Campfire Mug"
                url = "https://example.com/product"

                [[targets.conditions]]
                kind = "text_appears"
                value = "Add to cart"
            "#,
        )
        .expect("write targets");

        let (config, targets) =
            AppConfig::load(dir.path().join("config.toml").to_str().unwrap()).expect("load");

        assert!(config
            .targets_path
            .as_deref()
            .expect("targets path")
            .ends_with("targets.toml"));
        assert_eq!(targets.targets[0].id, "campfire");
    }

    #[test]
    fn targets_path_resolves_relative_to_config() {
        let dir = tempfile::tempdir().expect("tempdir");
        fs::create_dir(dir.path().join("subdir")).expect("mkdir");
        fs::write(
            dir.path().join("config.toml"),
            r#"targets_path = "subdir/targets.toml""#,
        )
        .expect("write config");
        fs::write(
            dir.path().join("subdir/targets.toml"),
            r#"
                [[targets]]
                id = "campfire"
                name = "Campfire Mug"
                url = "https://example.com/product"

                [[targets.conditions]]
                kind = "text_appears"
                value = "Add to cart"
            "#,
        )
        .expect("write targets");

        let (_, targets) =
            AppConfig::load(dir.path().join("config.toml").to_str().unwrap()).expect("load");

        assert_eq!(targets.targets[0].id, "campfire");
    }

    #[test]
    fn parses_cloakbrowser_renderer_config() {
        let raw = r#"
            [renderer]
            enabled = true
            backend = "cloakbrowser"
            endpoint = "http://cloakbrowser:9222"
            max_concurrency = 2
            navigation_timeout_ms = 30000
            operation_timeout_ms = 10000
            settle_ms = 750
        "#;

        let config = toml::from_str::<AppConfig>(raw).expect("parse config");
        config.validate().expect("valid renderer");

        assert!(config.renderer.enabled);
        assert_eq!(config.renderer.backend, RendererBackend::CloakBrowser);
        assert_eq!(
            config.renderer.endpoint.as_deref(),
            Some("http://cloakbrowser:9222")
        );
        assert_eq!(config.renderer.max_concurrency, 2);
    }

    #[test]
    fn maps_legacy_browser_config_to_renderer_when_renderer_absent() {
        let raw = r#"
            [browser]
            cdp_url = "ws://127.0.0.1:9222/devtools/browser/test"
            wait_ms = 2500
        "#;

        let config = toml::from_str::<AppConfig>(raw).expect("parse config");

        assert_eq!(config.renderer.backend, RendererBackend::Cdp);
        assert!(config.renderer.enabled);
        assert_eq!(
            config.renderer.endpoint.as_deref(),
            Some("ws://127.0.0.1:9222/devtools/browser/test")
        );
        assert_eq!(config.renderer.settle_ms, 2500);
    }

    #[test]
    fn rejects_enabled_renderer_without_endpoint() {
        let raw = r#"
            [renderer]
            enabled = true
        "#;
        let config = toml::from_str::<AppConfig>(raw).expect("parse config");

        let error = config.validate().expect_err("invalid renderer");
        assert!(error
            .to_string()
            .contains("enabled renderer requires endpoint"));
    }

    #[test]
    fn parses_bestbuy_render_first_policy() {
        let raw = r#"
            [[targets]]
            id = "bestbuy-airtag-open-box"
            name = "Best Buy AirTag Open-Box"
            url = "https://www.bestbuy.com/product/apple-airtag-4-pack-1st-generation-2021-silver/JJGCQ8XFQH/sku/6461349/openbox?condition=good"

            [targets.render]
            policy = "render_first"
            fingerprint_seed = "bestbuy-airtag-open-box"
            wait_ms = 3000

            [[targets.conditions]]
            kind = "text_appears"
            value = "Open-Box"
        "#;

        let targets = toml::from_str::<TargetsFile>(raw)
            .expect("parse targets")
            .resolve_and_validate()
            .expect("valid target");
        let target = &targets.targets[0];

        assert_eq!(target.render.policy, RenderPolicy::RenderFirst);
        assert_eq!(
            target.render.fingerprint_seed.as_deref(),
            Some("bestbuy-airtag-open-box")
        );
        assert_eq!(target.render.wait_ms, Some(3000));
    }

    #[test]
    fn parses_jellycat_steps_and_scenarios() {
        let raw = r#"
            [[targets]]
            id = "jellycat-bartholomew-bear"
            name = "Jellycat Bartholomew Bear"
            url = "https://us.jellycat.com/bartholomew-bear/"

            [targets.render]
            policy = "render_first"
            scenario_match = "any"

            [[targets.render.steps]]
            op = "wait_for_text"
            text = "Bartholomew Bear"

            [[targets.render.scenarios]]
            id = "medium"
            label = "Medium"

            [[targets.render.scenarios.steps]]
            op = "select"
            selector = "select[name*='Size']"
            option_text = "Medium"

            [[targets.conditions]]
            kind = "text_appears"
            value = "Bartholomew Bear"
        "#;

        let targets = toml::from_str::<TargetsFile>(raw)
            .expect("parse targets")
            .resolve_and_validate()
            .expect("valid render plan");

        assert_eq!(targets.targets[0].render.steps.len(), 1);
        assert_eq!(targets.targets[0].render.scenarios.len(), 1);
    }

    #[test]
    fn rejects_too_many_render_scenarios() {
        let scenarios = (1..=11)
            .map(|index| {
                format!("[[targets.render.scenarios]]\nid = \"s{index}\"\nlabel = \"S{index}\"\n")
            })
            .collect::<Vec<_>>()
            .join("\n");
        let raw = format!(
            r#"
            [[targets]]
            id = "too-many"
            name = "Too Many"
            url = "https://example.com/product"

            [targets.render]
            policy = "render_first"

            {scenarios}

            [[targets.conditions]]
            kind = "text_appears"
            value = "Available"
        "#
        );

        let error = toml::from_str::<TargetsFile>(&raw)
            .expect("parse targets")
            .resolve_and_validate()
            .expect_err("too many scenarios");

        assert!(error
            .to_string()
            .contains("render scenarios exceed the maximum"));
    }

    #[test]
    fn rejects_dangerous_render_step_text() {
        let raw = r#"
            [[targets]]
            id = "dangerous"
            name = "Dangerous"
            url = "https://example.com/product"

            [targets.render]
            policy = "render_first"

            [[targets.render.steps]]
            op = "click"
            selector = "button"
            text = "Add to cart"

            [[targets.conditions]]
            kind = "text_appears"
            value = "Available"
        "#;

        let error = toml::from_str::<TargetsFile>(raw)
            .expect("parse targets")
            .resolve_and_validate()
            .expect_err("dangerous step");

        assert!(error
            .to_string()
            .contains("render step targets a dangerous action"));
    }

    #[test]
    fn render_step_operation_uses_snake_case_wire_values() {
        let raw = r#"
            op = "wait_for_text"
            text = "Ready"
        "#;
        let step = toml::from_str::<super::RenderStep>(raw).expect("parse render step");
        assert_eq!(step.op, RenderOperation::WaitForText);
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
            assert_eq!(condition.kind(), kind);
            assert_eq!(condition.negate(), negate);
        }
    }

    #[test]
    fn rejects_text_condition_without_value() {
        let error = validate_single_condition("kind = \"text_appears\"").expect_err("invalid");
        assert!(error
            .to_string()
            .contains("condition condition-1 requires value"));
    }

    #[test]
    fn rejects_selector_condition_without_selector() {
        let error = validate_single_condition("kind = \"selector_exists\"").expect_err("invalid");
        assert!(error
            .to_string()
            .contains("condition condition-1 requires selector"));
    }

    #[test]
    fn rejects_selector_text_condition_without_selector() {
        let error = validate_single_condition(
            r#"
            kind = "selector_text_contains"
            value = "Add to cart"
            "#,
        )
        .expect_err("invalid");
        assert!(error
            .to_string()
            .contains("condition condition-1 requires selector"));
    }

    #[test]
    fn rejects_selector_text_condition_without_value() {
        let error = validate_single_condition(
            r#"
            kind = "selector_text_contains"
            selector = "button"
            "#,
        )
        .expect_err("invalid");
        assert!(error
            .to_string()
            .contains("condition condition-1 requires value"));
    }

    #[test]
    fn rejects_price_condition_without_threshold() {
        let error = validate_single_condition("kind = \"price_below\"").expect_err("invalid");
        assert!(error
            .to_string()
            .contains("condition condition-1 requires threshold_cents"));
    }

    #[test]
    fn accepts_price_changed_without_price_selector() {
        let targets = validate_single_condition("kind = \"price_changed\"").expect("valid");
        assert_eq!(
            targets.targets[0].conditions[0].kind(),
            ConditionKind::PriceObserved
        );
    }

    fn validate_single_condition(condition: &str) -> Result<TargetsFile, crate::Error> {
        toml::from_str::<TargetsFile>(&format!(
            r#"
            [[targets]]
            id = "target"
            name = "Target"
            url = "https://example.com/product"

            [[targets.conditions]]
            {condition}
            "#
        ))
        .expect("parse targets")
        .resolve_and_validate()
    }

    #[test]
    fn serializes_back_to_legacy_strings() {
        let condition = Condition {
            id: Some("gone".to_string()),
            rule: ConditionRule::Text {
                value: "Add to cart".to_string(),
                negate: true,
            },
        };

        let encoded = toml::to_string(&condition).expect("serialize condition");
        assert!(encoded.contains("kind = \"text_disappears\""));
    }
}
