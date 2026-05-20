use serde::Serialize;
use snafu::{ensure, OptionExt, ResultExt};

use crate::{
    config::AppConfig,
    config::{CheckOutcome, TargetStatus},
    error::{DiscordStatusSnafu, MissingDiscordWebhookSnafu, RequestSnafu, Result},
};

#[derive(Debug, Serialize)]
struct DiscordMessage {
    content: String,
    embeds: Vec<DiscordEmbed>,
}

#[derive(Debug, Serialize)]
struct DiscordEmbed {
    title: String,
    url: String,
    description: String,
    fields: Vec<DiscordField>,
}

#[derive(Debug, Serialize)]
struct DiscordField {
    name: String,
    value: String,
    inline: bool,
}

pub async fn send_condition_alert(
    client: &reqwest::Client,
    config: &AppConfig,
    outcome: &CheckOutcome,
) -> Result<()> {
    send_message(
        client,
        config,
        &DiscordMessage {
            content: format!(
                "{} matched its webwatch alert conditions",
                outcome.target.name
            ),
            embeds: vec![DiscordEmbed {
                title: outcome.target.name.clone(),
                url: outcome.target.url.clone(),
                description: evidence_text(&outcome.evidence),
                fields: vec![
                    DiscordField {
                        name: "Engine".to_string(),
                        value: format!("{:?}", outcome.engine_used),
                        inline: true,
                    },
                    DiscordField {
                        name: "Price".to_string(),
                        value: outcome
                            .price_cents
                            .map(format_price)
                            .unwrap_or_else(|| "unknown".to_string()),
                        inline: true,
                    },
                ],
            }],
        },
    )
    .await
}

pub async fn send_status_report(
    client: &reqwest::Client,
    config: &AppConfig,
    summary: &str,
) -> Result<()> {
    send_message(
        client,
        config,
        &DiscordMessage {
            content: "webwatch status requested manually".to_string(),
            embeds: vec![DiscordEmbed {
                title: "webwatch status".to_string(),
                url: "https://example.invalid".to_string(),
                description: summary.to_string(),
                fields: vec![],
            }],
        },
    )
    .await
}

pub fn render_status_report(statuses: &[TargetStatus]) -> String {
    if statuses.is_empty() {
        return "No targets configured.".to_string();
    }

    let matched = statuses
        .iter()
        .filter(|status| status.matched == Some(true))
        .count();
    let errors = statuses
        .iter()
        .filter(|status| status.last_error.is_some())
        .count();
    let header = format!(
        "Checked {} target(s): {matched} matched, {errors} error(s).",
        statuses.len()
    );

    let targets = statuses
        .iter()
        .map(render_target_status)
        .collect::<Vec<_>>()
        .join("\n\n");

    format!("{header}\n\n{targets}")
}

fn render_target_status(status: &TargetStatus) -> String {
    let icon = if status.last_error.is_some() {
        "⚠️"
    } else if status.matched == Some(true) {
        "🚨"
    } else {
        "✅"
    };
    let state = match status.matched {
        Some(true) => "matched",
        Some(false) => "not matched",
        None => "unknown",
    };
    let checked = status.last_success_at.as_deref().unwrap_or("never");
    let engine = status
        .engine_used
        .map(|engine| format!("{engine:?}"))
        .unwrap_or_else(|| "unknown".to_string());
    let price = status
        .price_cents
        .map(format_price)
        .unwrap_or_else(|| "unknown".to_string());
    let condition_summary = condition_summary(status);
    let detail = status
        .last_error
        .as_deref()
        .map(|error| format!("Error: {error}"))
        .unwrap_or_else(|| first_evidence(&status.evidence));

    format!(
        "{icon} **{}** — {state}\nURL: {}\nLast check: {checked}\nEngine: {engine} · Price: {price}\nConditions: {condition_summary}\n{detail}",
        status.name, status.url
    )
}

fn condition_summary(status: &TargetStatus) -> String {
    if status.condition_results.is_empty() {
        return "unknown".to_string();
    }

    let matched = status
        .condition_results
        .iter()
        .filter(|condition| condition.matched)
        .count();
    format!("{matched}/{} matched", status.condition_results.len())
}

fn first_evidence(evidence: &[String]) -> String {
    evidence
        .first()
        .cloned()
        .unwrap_or_else(|| "Evidence: none recorded".to_string())
}

async fn send_message(
    client: &reqwest::Client,
    config: &AppConfig,
    message: &DiscordMessage,
) -> Result<()> {
    let webhook_url = config
        .discord_webhook_url
        .as_ref()
        .context(MissingDiscordWebhookSnafu)?;
    let response = client
        .post(webhook_url)
        .json(message)
        .send()
        .await
        .context(RequestSnafu {
            url: webhook_url.to_string(),
        })?;
    let status = response.status();
    if status.is_success() {
        return Ok(());
    }
    let body = response
        .text()
        .await
        .unwrap_or_else(|_| "<failed to read body>".to_string());
    ensure!(status.is_success(), DiscordStatusSnafu { status, body });
    Ok(())
}

fn evidence_text(evidence: &[String]) -> String {
    if evidence.is_empty() {
        "no evidence recorded".to_string()
    } else {
        evidence.join("\n")
    }
}

fn format_price(cents: i64) -> String {
    format!("${:.2}", cents as f64 / 100.0)
}

#[cfg(test)]
mod tests {
    use super::render_status_report;
    use crate::config::{ConditionKind, ConditionResult, EngineUsed, TargetStatus};

    #[test]
    fn status_report_includes_counts_url_and_condition_summary() {
        let report = render_status_report(&[TargetStatus {
            target_id: "mug".to_string(),
            name: "Campfire Mug".to_string(),
            url: "https://example.com/mug".to_string(),
            matched: Some(true),
            engine_used: Some(EngineUsed::Http),
            price_cents: Some(4250),
            evidence: vec!["page text contains 'Add to cart'".to_string()],
            condition_results: vec![ConditionResult {
                condition_id: "stock".to_string(),
                kind: ConditionKind::TextAppears,
                matched: true,
                evidence: vec![],
                observed_price_cents: None,
                error: None,
            }],
            last_success_at: Some("2026-05-18T12:00:00Z".to_string()),
            last_error_at: None,
            last_error: None,
            last_alert_at: None,
        }]);

        assert!(report.contains("Checked 1 target(s): 1 matched, 0 error(s)."));
        assert!(report.contains("URL: https://example.com/mug"));
        assert!(report.contains("Conditions: 1/1 matched"));
        assert!(report.contains("$42.50"));
    }
}
