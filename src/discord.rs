use serde::Serialize;
use snafu::{ensure, OptionExt, ResultExt};

use crate::{
    config::AppConfig,
    config::{CheckOutcome, EngineUsed, TargetStatus},
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
}

pub async fn send_condition_alert(
    client: &reqwest::Client,
    config: &AppConfig,
    outcome: &CheckOutcome,
) -> Result<()> {
    let message = condition_alert_message(outcome);
    send_message(client, config, &message).await
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
            content: summary.to_string(),
            embeds: vec![],
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

fn condition_alert_message(outcome: &CheckOutcome) -> DiscordMessage {
    let first_evidence = outcome
        .evidence
        .first()
        .map(String::as_str)
        .unwrap_or("condition matched");
    DiscordMessage {
        content: format!("🚨 {} — {first_evidence}", outcome.target.name),
        embeds: vec![DiscordEmbed {
            title: outcome.target.name.clone(),
            url: outcome.target.url.clone(),
            description: condition_alert_description(outcome),
        }],
    }
}

fn condition_alert_description(outcome: &CheckOutcome) -> String {
    let mut lines = outcome
        .evidence
        .iter()
        .skip(1)
        .take(5)
        .cloned()
        .collect::<Vec<_>>();
    if outcome.engine_used == EngineUsed::BrowserCdp {
        lines.push("Engine: BrowserCdp".to_string());
    }
    if let Some(price) = outcome.price_cents {
        lines.push(format!("Price: {}", format_price(price)));
    }
    truncate(&lines.join("\n"), 1024)
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
    let checked = status.last_success_at.as_deref().unwrap_or("never checked");
    let detail = status
        .last_error
        .as_deref()
        .map(|error| format!("Error: {error}"))
        .unwrap_or_else(|| first_evidence(&status.evidence));

    format!(
        "{icon} **{}** — {state}\n{}\n{checked} · {detail}",
        status.name, status.url
    )
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

fn truncate(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn format_price(cents: i64) -> String {
    format!("${:.2}", cents as f64 / 100.0)
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::{condition_alert_message, render_status_report};
    use crate::config::{
        CheckOutcome, ConditionKind, ConditionResult, EngineUsed, Target, TargetStatus,
    };

    #[test]
    fn status_report_includes_counts_url_and_first_evidence() {
        let report = render_status_report(&[TargetStatus {
            target_id: "mug".to_string(),
            name: "Campfire Mug".to_string(),
            url: "https://example.com/mug".to_string(),
            enabled: true,
            matched: Some(true),
            engine_used: Some(EngineUsed::Http),
            price_cents: Some(4250),
            evidence: vec!["page text contains 'Add to cart'".to_string()],
            condition_results: vec![ConditionResult {
                condition_id: "stock".to_string(),
                kind: ConditionKind::Text,
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
        assert!(report.contains("Campfire Mug"));
        assert!(report.contains("https://example.com/mug"));
        assert!(report.contains("2026-05-18T12:00:00Z · page text contains 'Add to cart'"));
        assert!(!report.contains("Conditions:"));
        assert!(!report.contains("Price:"));
    }

    #[test]
    fn condition_alert_uses_target_url_as_embed_url() {
        let message = condition_alert_message(&CheckOutcome {
            target: Target {
                id: "mug".to_string(),
                name: "Campfire Mug".to_string(),
                url: "https://example.com/mug".to_string(),
                enabled: true,
                interval_secs: None,
                conditions: vec![],
            },
            engine_used: EngineUsed::Http,
            matched: true,
            checked_at: Utc::now(),
            price_cents: None,
            evidence: vec!["page text contains 'Add to cart'".to_string()],
            condition_results: vec![],
        });

        assert_eq!(message.embeds[0].url, "https://example.com/mug");
        assert!(!message.embeds[0].url.contains("example.invalid"));
    }
}
