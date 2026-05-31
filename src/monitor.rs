use crate::{
    config::{AppConfig, CheckOutcome, Target},
    db::Persistence,
    evaluator, Result,
};

/// Result of running and recording a single check.
pub enum CheckReport {
    /// The page was fetched and evaluated, and the outcome was recorded.
    /// `should_alert` is true when the target newly transitioned into a match.
    Checked {
        outcome: CheckOutcome,
        should_alert: bool,
    },
    /// The check itself failed (network, HTTP status, browser required, …).
    /// The error was recorded against the target.
    Failed { error: String },
}

/// Run one check against `target` and record the result. A returned `Err` means
/// persistence failed; a failed *check* is recorded and surfaced as
/// [`CheckReport::Failed`] so callers can keep going.
pub async fn run_check(
    config: &AppConfig,
    db: &dyn Persistence,
    client: &reqwest::Client,
    target: Target,
) -> Result<CheckReport> {
    let target_id = target.id.clone();
    match evaluator::check_target(config, client, target).await {
        Ok(outcome) => {
            let should_alert = db.record_success(&outcome).await?;
            Ok(CheckReport::Checked {
                outcome,
                should_alert,
            })
        }
        Err(error) => {
            let error_text = error.to_string();
            db.record_error(&target_id, &error_text).await?;
            Ok(CheckReport::Failed { error: error_text })
        }
    }
}
