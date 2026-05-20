use std::{sync::Arc, time::Duration};

use rand::Rng;
use tracing::{error, info, warn};

use crate::{config::AppConfig, config::Target, db::Persistence, discord, evaluator, Result};

pub fn spawn_all(config: Arc<AppConfig>, db: Arc<dyn Persistence>, client: reqwest::Client) {
    for target_config in config
        .targets
        .iter()
        .filter(|target| target.enabled())
        .cloned()
    {
        let config = config.clone();
        let db = db.clone();
        let client = client.clone();
        tokio::spawn(async move {
            if let Err(error) = run_target_loop(config, db, client, target_config).await {
                error!(%error, "target loop stopped");
            }
        });
    }
}

async fn run_target_loop(
    config: Arc<AppConfig>,
    db: Arc<dyn Persistence>,
    client: reqwest::Client,
    target_config: crate::config::TargetConfig,
) -> Result<()> {
    let target = target_config.to_target()?;
    let interval_secs = target_config.interval_secs(&config);
    info!(target_id = %target.id, interval_secs, "starting target checker");

    loop {
        run_once(&config, db.as_ref(), &client, target.clone()).await;
        tokio::time::sleep(next_delay(interval_secs, config.scheduler.jitter_secs)).await;
    }
}

async fn run_once(
    config: &AppConfig,
    db: &dyn Persistence,
    client: &reqwest::Client,
    target: Target,
) {
    match evaluator::check_target(config, client, target.clone()).await {
        Ok(outcome) => match db.record_success(&outcome).await {
            Ok(should_alert) => {
                info!(
                    target_id = %outcome.target.id,
                    matched = outcome.matched,
                    engine = ?outcome.engine_used,
                    price_cents = ?outcome.price_cents,
                    should_alert,
                    "check succeeded"
                );
                if should_alert {
                    match discord::send_condition_alert(client, config, &outcome).await {
                        Ok(()) => {
                            if let Err(error) = db.mark_alert_sent(&outcome.target.id).await {
                                warn!(%error, target_id = %outcome.target.id, "failed to record alert timestamp");
                            }
                        }
                        Err(error) => {
                            warn!(%error, target_id = %outcome.target.id, "failed to send discord alert")
                        }
                    }
                }
            }
            Err(error) => {
                error!(%error, target_id = %target.id, "failed to record successful check")
            }
        },
        Err(error) => {
            let error_text = error.to_string();
            warn!(error = %error_text, target_id = %target.id, "check failed");
            if let Err(record_error) = db.record_error(&target.id, &error_text).await {
                error!(%record_error, target_id = %target.id, "failed to record check error");
            }
        }
    }
}

fn next_delay(interval_secs: u64, jitter_secs: u64) -> Duration {
    if jitter_secs == 0 {
        return Duration::from_secs(interval_secs.max(1));
    }

    let jitter = rand::thread_rng().gen_range(-(jitter_secs as i64)..=(jitter_secs as i64));
    let secs = (interval_secs as i64 + jitter).max(1) as u64;
    Duration::from_secs(secs)
}

#[cfg(test)]
mod tests {
    use super::next_delay;

    #[test]
    fn delay_never_zero() {
        for _ in 0..100 {
            assert!(next_delay(1, 30).as_secs() >= 1);
        }
    }
}
