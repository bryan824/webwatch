use chrono::Utc;
use regex::Regex;
use scraper::{Html, Selector};
use snafu::{ensure, ResultExt};

use crate::{
    browser,
    config::AppConfig,
    config::{CheckOutcome, Condition, ConditionKind, ConditionResult, EngineUsed, Target},
    error::{BrowserRequiredSnafu, HttpStatusSnafu, RequestSnafu, Result},
};

pub async fn check_target(
    config: &AppConfig,
    client: &reqwest::Client,
    target: Target,
) -> Result<CheckOutcome> {
    match check_with_http(config, client, target.clone()).await {
        Ok(outcome) => Ok(outcome),
        Err(crate::Error::BrowserRequired { .. }) if config.browser.cdp_url.is_some() => {
            browser::check_with_browser(config, target).await
        }
        Err(error) => Err(error),
    }
}

async fn check_with_http(
    config: &AppConfig,
    client: &reqwest::Client,
    target: Target,
) -> Result<CheckOutcome> {
    let response = client
        .get(&target.url)
        .header(reqwest::header::USER_AGENT, &config.user_agent)
        .send()
        .await
        .context(RequestSnafu {
            url: target.url.clone(),
        })?;
    let status = response.status();
    ensure!(
        status.is_success(),
        HttpStatusSnafu {
            url: target.url.clone(),
            status
        }
    );
    let body = response.text().await.context(RequestSnafu {
        url: target.url.clone(),
    })?;

    evaluate_document(target, EngineUsed::Http, &body)
}

pub fn evaluate_document(
    target: Target,
    engine_used: EngineUsed,
    body: &str,
) -> Result<CheckOutcome> {
    let document = Html::parse_document(body);
    let page_text =
        normalize_whitespace(&document.root_element().text().collect::<Vec<_>>().join(" "));
    let looks_js_rendered = looks_like_js_shell(body, &page_text);

    let mut condition_results = Vec::with_capacity(target.conditions.len());
    let mut evidence = Vec::new();
    let mut price_cents = None;
    let mut needs_browser_reasons = Vec::new();

    for condition in &target.conditions {
        let result = evaluate_condition(condition, &document, &page_text, body)?;
        if !result.matched && should_try_browser(condition, looks_js_rendered, &result) {
            needs_browser_reasons.push(format!("{} not proven by HTTP", condition_id(condition)));
        }
        if price_cents.is_none() {
            price_cents = result.observed_price_cents;
        }
        evidence.extend(result.evidence.iter().cloned());
        condition_results.push(result);
    }

    if !needs_browser_reasons.is_empty() && engine_used == EngineUsed::Http {
        return BrowserRequiredSnafu {
            reason: needs_browser_reasons.join("; "),
        }
        .fail();
    }

    let matched = condition_results.iter().all(|result| result.matched);
    Ok(CheckOutcome {
        target,
        engine_used,
        matched,
        checked_at: Utc::now(),
        price_cents,
        evidence: evidence.into_iter().take(10).collect(),
        condition_results,
    })
}

fn evaluate_condition(
    condition: &Condition,
    document: &Html,
    page_text: &str,
    body: &str,
) -> Result<ConditionResult> {
    let mut evidence = Vec::new();
    let mut observed_price_cents = None;
    let base_matched = match condition.kind {
        ConditionKind::Text => {
            let value = required_value(condition)?;
            let found = contains_case_insensitive(page_text, value);
            if found {
                evidence.push(format!("page text contains '{value}'"));
            } else if condition.negate {
                evidence.push(format!("page text does not contain '{value}'"));
            }
            found
        }
        ConditionKind::Selector => {
            let selector = required_selector(condition)?;
            let count = select_texts(document, selector)?.len();
            if count > 0 {
                evidence.push(format!("selector '{selector}' matched {count} element(s)"));
            } else if condition.negate {
                evidence.push(format!("selector '{selector}' did not match"));
            }
            count > 0
        }
        ConditionKind::SelectorText => {
            let selector = required_selector(condition)?;
            let value = required_value(condition)?;
            let texts = select_texts(document, selector)?;
            let found_text = texts
                .iter()
                .find(|text| contains_case_insensitive(text, value));
            if let Some(text) = found_text {
                evidence.push(format!(
                    "selector '{selector}' text contains '{value}': {text}"
                ));
                true
            } else {
                if condition.negate {
                    evidence.push(format!(
                        "selector '{selector}' text does not contain '{value}'"
                    ));
                }
                false
            }
        }
        ConditionKind::Price => {
            let threshold = required_threshold(condition)?;
            observed_price_cents = extract_price_cents(document, page_text, condition)?;
            let below = observed_price_cents
                .map(|price| price < threshold)
                .unwrap_or(false);
            if let Some(price) = observed_price_cents {
                if condition.negate {
                    evidence.push(format!(
                        "observed price {} is above {}",
                        money(price),
                        money(threshold)
                    ));
                } else {
                    evidence.push(format!(
                        "observed price {} is below {}",
                        money(price),
                        money(threshold)
                    ));
                }
            }
            below
        }
        ConditionKind::PriceObserved => {
            observed_price_cents = extract_price_cents(document, page_text, condition)?;
            let matched = observed_price_cents.is_some();
            if let Some(price) = observed_price_cents {
                evidence.push(format!("observed price {}", money(price)));
            }
            matched
        }
    };
    let matched = if condition.negate {
        !base_matched
    } else {
        base_matched
    };

    if evidence.is_empty() && body.is_empty() {
        evidence.push("empty response body".to_string());
    }

    Ok(ConditionResult {
        condition_id: condition_id(condition),
        kind: condition.kind,
        matched,
        evidence,
        observed_price_cents,
        error: None,
    })
}

fn condition_id(condition: &Condition) -> String {
    condition
        .id
        .clone()
        .unwrap_or_else(|| "condition".to_string())
}

fn required_value(condition: &Condition) -> Result<&str> {
    condition
        .value
        .as_deref()
        .ok_or_else(|| crate::Error::MissingConditionField {
            condition_id: condition_id(condition),
            field: "value",
        })
}

fn required_selector(condition: &Condition) -> Result<&str> {
    condition
        .selector
        .as_deref()
        .ok_or_else(|| crate::Error::MissingConditionField {
            condition_id: condition_id(condition),
            field: "selector",
        })
}

fn required_threshold(condition: &Condition) -> Result<i64> {
    condition
        .threshold_cents
        .ok_or_else(|| crate::Error::MissingConditionField {
            condition_id: condition_id(condition),
            field: "threshold_cents",
        })
}

fn select_texts(document: &Html, selector: &str) -> Result<Vec<String>> {
    let selector = Selector::parse(selector).map_err(|error| crate::Error::InvalidSelector {
        selector: selector.to_string(),
        message: error.to_string(),
    })?;
    Ok(document
        .select(&selector)
        .map(|element| normalize_whitespace(&element.text().collect::<Vec<_>>().join(" ")))
        .collect())
}

fn extract_price_cents(
    document: &Html,
    page_text: &str,
    condition: &Condition,
) -> Result<Option<i64>> {
    let text = if let Some(selector) = &condition.price_selector {
        select_texts(document, selector)?.join(" ")
    } else if let Some(selector) = &condition.selector {
        let selected = select_texts(document, selector)?.join(" ");
        if selected.is_empty() {
            page_text.to_string()
        } else {
            selected
        }
    } else {
        page_text.to_string()
    };
    Ok(first_price_cents(&text))
}

fn first_price_cents(text: &str) -> Option<i64> {
    let regex = Regex::new(r"\$\s*([0-9]{1,3}(?:,[0-9]{3})*|[0-9]+)(?:\.([0-9]{1,2}))?").ok()?;
    let captures = regex.captures(text)?;
    let dollars = captures.get(1)?.as_str().replace(',', "");
    let dollars = dollars.parse::<i64>().ok()?;
    let cents = captures
        .get(2)
        .map(|value| format!("{:<0width$}", value.as_str(), width = 2))
        .and_then(|value| value.get(0..2).map(str::to_string))
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(0);
    Some(dollars * 100 + cents)
}

fn contains_case_insensitive(haystack: &str, needle: &str) -> bool {
    haystack.to_lowercase().contains(&needle.to_lowercase())
}

fn normalize_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn looks_like_js_shell(body: &str, page_text: &str) -> bool {
    let script_count = body.matches("<script").count();
    script_count > 0 && page_text.len() < 200
}

fn should_try_browser(
    condition: &Condition,
    looks_js_rendered: bool,
    result: &ConditionResult,
) -> bool {
    if result.matched || !looks_js_rendered {
        return false;
    }
    !condition.negate
        && matches!(
            condition.kind,
            ConditionKind::Text
                | ConditionKind::Selector
                | ConditionKind::SelectorText
                | ConditionKind::Price
                | ConditionKind::PriceObserved
        )
}

fn money(cents: i64) -> String {
    format!("${:.2}", cents as f64 / 100.0)
}

#[cfg(test)]
mod tests {
    use super::{evaluate_document, first_price_cents};
    use crate::config::{Condition, ConditionKind, EngineUsed, Target};

    fn target(condition: Condition) -> Target {
        Target {
            id: "target".to_string(),
            name: "Target".to_string(),
            url: "https://example.com/product".to_string(),
            enabled: true,
            interval_secs: None,
            conditions: vec![condition],
        }
    }

    #[test]
    fn extracts_first_price() {
        assert_eq!(first_price_cents("Now only $1,234.50"), Some(123_450));
        assert_eq!(first_price_cents("Price $99"), Some(9_900));
    }

    #[test]
    fn evaluates_selector_text_condition() {
        let condition = Condition {
            id: Some("stock".to_string()),
            kind: ConditionKind::SelectorText,
            negate: false,
            value: Some("Add to cart".to_string()),
            selector: Some("button".to_string()),
            threshold_cents: None,
            price_selector: None,
        };
        let outcome = evaluate_document(
            target(condition),
            EngineUsed::Http,
            "<html><body><button>Add to cart</button></body></html>",
        )
        .expect("evaluate");

        assert!(outcome.matched);
        assert_eq!(outcome.engine_used, EngineUsed::Http);
    }

    #[test]
    fn negates_text_condition() {
        let condition = Condition {
            id: Some("gone".to_string()),
            kind: ConditionKind::Text,
            negate: true,
            value: Some("Add to cart".to_string()),
            selector: None,
            threshold_cents: None,
            price_selector: None,
        };
        let outcome = evaluate_document(
            target(condition),
            EngineUsed::Http,
            "<html><body>Sold out</body></html>",
        )
        .expect("evaluate");

        assert!(outcome.matched);
        assert_eq!(
            outcome.evidence[0],
            "page text does not contain 'Add to cart'"
        );
    }
}
