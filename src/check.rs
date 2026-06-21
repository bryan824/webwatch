use chrono::Utc;

use crate::{
    config::{
        AppConfig, CheckOutcome, ConditionResult, EngineUsed, RenderPolicy, ScenarioMatch, Target,
    },
    evaluator,
    renderer::{RenderRequest, RenderedSnapshot, RendererService},
    Result,
};

pub async fn check_target(
    config: &AppConfig,
    client: &reqwest::Client,
    renderer: &RendererService,
    target: Target,
) -> Result<CheckOutcome> {
    match target.render.policy {
        RenderPolicy::HttpOnly => evaluator::check_with_http(config, client, target).await,
        RenderPolicy::RenderFirst => check_with_renderer(config, renderer, target).await,
        RenderPolicy::Auto => {
            match evaluator::check_with_http(config, client, target.clone()).await {
                Ok(outcome) => Ok(outcome),
                Err(error) if renderer.is_available() && http_failure_allows_render(&error) => {
                    check_with_renderer(config, renderer, target).await
                }
                Err(error) => Err(error),
            }
        }
    }
}

/// HTTP failures a browser render might still recover from: a page that needs
/// JavaScript (`BrowserRequired`), a connection the origin refused such as bot
/// blocking (`Request`), or a non-success status (`HttpStatus`). Other errors
/// surface as-is so a genuine outage is not masked by a wasted render attempt.
fn http_failure_allows_render(error: &crate::Error) -> bool {
    matches!(
        error,
        crate::Error::BrowserRequired { .. }
            | crate::Error::Request { .. }
            | crate::Error::HttpStatus { .. }
    )
}

async fn check_with_renderer(
    config: &AppConfig,
    renderer: &RendererService,
    target: Target,
) -> Result<CheckOutcome> {
    let snapshots = renderer
        .render_snapshots(RenderRequest {
            target_id: target.id.clone(),
            url: target.url.clone(),
            plan: target.render.clone(),
        })
        .await?;
    persist_last_render(config, &target.id, &snapshots);
    if let Some(nav_error) = snapshots
        .iter()
        .find_map(|snapshot| snapshot.nav_error.clone())
    {
        return Err(crate::Error::Browser {
            stage: "navigate",
            message: format!("navigation to {} failed: {nav_error}", target.url),
        });
    }
    evaluate_snapshots(target, snapshots)
}

/// Persist the last rendered snapshot (HTML + screenshot) next to the database
/// so it can be exported for debugging — including error pages, which is exactly
/// when it matters. Best-effort: a write failure never fails the check.
fn persist_last_render(config: &AppConfig, target_id: &str, snapshots: &[RenderedSnapshot]) {
    let Some(snapshot) = snapshots.first() else {
        return;
    };
    let dir = config.snapshot_dir(target_id);
    if let Err(error) = std::fs::create_dir_all(&dir) {
        tracing::warn!(target_id, %error, "could not create snapshot directory");
        return;
    }
    if let Err(error) = std::fs::write(dir.join("last.html"), &snapshot.html) {
        tracing::warn!(target_id, %error, "could not write last render HTML");
    }
    if let Some(png) = &snapshot.screenshot_png {
        if let Err(error) = std::fs::write(dir.join("last.png"), png) {
            tracing::warn!(target_id, %error, "could not write last render screenshot");
        }
    }
}

fn evaluate_snapshots(target: Target, snapshots: Vec<RenderedSnapshot>) -> Result<CheckOutcome> {
    if snapshots.len() == 1 && snapshots[0].scenario_id.is_none() {
        let snapshot = &snapshots[0];
        let mut outcome = evaluator::evaluate_document(
            target,
            EngineUsed::BrowserCdp,
            &snapshot.html,
            &snapshot.final_url,
        )?;
        outcome.evidence = snapshot
            .evidence
            .iter()
            .cloned()
            .chain(outcome.evidence)
            .take(10)
            .collect();
        return Ok(outcome);
    }

    let mut scenario_outcomes = Vec::with_capacity(snapshots.len());
    for snapshot in &snapshots {
        let mut outcome = evaluator::evaluate_document(
            target.clone(),
            EngineUsed::BrowserCdp,
            &snapshot.html,
            &snapshot.final_url,
        )?;
        apply_scenario_to_results(
            &mut outcome.condition_results,
            snapshot.scenario_id.clone(),
            snapshot.scenario_label.clone(),
        );
        outcome.evidence = snapshot
            .evidence
            .iter()
            .cloned()
            .chain(label_evidence(snapshot, outcome.evidence))
            .take(10)
            .collect();
        scenario_outcomes.push(outcome);
    }

    let matched = match target.render.scenario_match {
        ScenarioMatch::Any => scenario_outcomes.iter().any(|outcome| outcome.matched),
        ScenarioMatch::All => scenario_outcomes.iter().all(|outcome| outcome.matched),
    };
    let price_cents = scenario_outcomes
        .iter()
        .find_map(|outcome| outcome.price_cents);
    let mut evidence = Vec::new();
    let mut condition_results = Vec::new();
    for outcome in scenario_outcomes {
        evidence.extend(outcome.evidence);
        condition_results.extend(outcome.condition_results);
    }

    Ok(CheckOutcome {
        target,
        engine_used: EngineUsed::BrowserCdp,
        matched,
        checked_at: Utc::now(),
        price_cents,
        evidence: evidence.into_iter().take(10).collect(),
        condition_results,
    })
}

fn apply_scenario_to_results(
    condition_results: &mut [ConditionResult],
    scenario_id: Option<String>,
    scenario_label: Option<String>,
) {
    for result in condition_results {
        result.scenario_id = scenario_id.clone();
        result.scenario_label = scenario_label.clone();
    }
}

fn label_evidence(snapshot: &RenderedSnapshot, evidence: Vec<String>) -> Vec<String> {
    let Some(label) = &snapshot.scenario_label else {
        return evidence;
    };
    evidence
        .into_iter()
        .map(|entry| format!("{label}: {entry}"))
        .collect()
}
