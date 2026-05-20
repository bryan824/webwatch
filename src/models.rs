use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use snafu::{ensure, ResultExt};
use url::Url;

use crate::error::{EmptyConditionsSnafu, ParseTargetUrlSnafu, Result};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Target {
    pub id: String,
    pub name: String,
    pub url: String,
    pub conditions: Vec<Condition>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Condition {
    pub id: String,
    pub kind: ConditionKind,
    pub value: Option<String>,
    pub selector: Option<String>,
    pub threshold_cents: Option<i64>,
    pub price_selector: Option<String>,
}

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

impl Target {
    pub fn from_config(config: &crate::config::TargetConfig) -> Result<Self> {
        Url::parse(&config.url).context(ParseTargetUrlSnafu {
            target_id: config.id.clone(),
        })?;
        ensure!(
            !config.conditions.is_empty(),
            EmptyConditionsSnafu {
                target_id: config.id.clone()
            }
        );

        let conditions = config
            .conditions
            .iter()
            .enumerate()
            .map(|(index, condition)| Condition {
                id: condition
                    .id
                    .clone()
                    .unwrap_or_else(|| format!("condition-{}", index + 1)),
                kind: condition.kind,
                value: condition.value.clone(),
                selector: condition.selector.clone(),
                threshold_cents: condition.threshold_cents,
                price_selector: condition.price_selector.clone(),
            })
            .collect();

        Ok(Self {
            id: config.id.clone(),
            name: config.name.clone(),
            url: config.url.clone(),
            conditions,
        })
    }
}

impl CheckOutcome {
    pub fn condition_met(&self) -> bool {
        self.matched
    }
}

#[cfg(test)]
mod tests {
    use super::Target;
    use crate::{
        config::{ConditionConfig, TargetConfig},
        models::ConditionKind,
    };

    #[test]
    fn builds_generic_target_from_url_and_conditions() {
        let target = Target::from_config(&TargetConfig {
            id: "campfire".to_string(),
            name: "Campfire Mug".to_string(),
            url: "https://example.com/product".to_string(),
            enabled: Some(true),
            interval_secs: None,
            conditions: vec![ConditionConfig {
                id: None,
                kind: ConditionKind::TextAppears,
                value: Some("Add to cart".to_string()),
                selector: None,
                threshold_cents: None,
                price_selector: None,
            }],
        })
        .expect("valid target");

        assert_eq!(target.url, "https://example.com/product");
        assert_eq!(target.conditions[0].id, "condition-1");
    }
}
