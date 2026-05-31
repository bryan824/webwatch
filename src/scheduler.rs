use std::{collections::HashMap, sync::Arc, time::Duration};

use rand::Rng;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use crate::{
    config::{AppConfig, CheckOutcome, Target},
    db::Persistence,
    discord, monitor, Result,
};

#[derive(Clone)]
pub struct Scheduler {
    inner: Arc<Mutex<SchedulerInner>>,
    config: Arc<AppConfig>,
    db: Arc<dyn Persistence>,
    client: reqwest::Client,
}

#[derive(Default)]
struct SchedulerInner {
    tasks: HashMap<String, RunningTarget>,
}

struct RunningTarget {
    target: Target,
    handle: tokio::task::JoinHandle<()>,
}

impl Scheduler {
    pub fn new(config: Arc<AppConfig>, db: Arc<dyn Persistence>, client: reqwest::Client) -> Self {
        Self {
            inner: Arc::new(Mutex::new(SchedulerInner::default())),
            config,
            db,
            client,
        }
    }

    pub async fn start(&self, targets: &[Target]) {
        let mut inner = self.inner.lock().await;
        for target in targets.iter().filter(|target| target.enabled()).cloned() {
            let id = target.id.clone();
            let handle = self.spawn_target(target.clone());
            inner.tasks.insert(id, RunningTarget { target, handle });
        }
    }

    pub async fn reconcile_target(&self, target: Target) {
        let mut inner = self.inner.lock().await;
        self.reconcile(&mut inner, target);
    }

    pub async fn remove_running_target(&self, id: &str) -> bool {
        let mut inner = self.inner.lock().await;
        if let Some(running) = inner.tasks.remove(id) {
            running.handle.abort();
            return true;
        }
        false
    }

    /// Start, restart, or stop a target's loop to match its `enabled` flag.
    fn reconcile(&self, inner: &mut SchedulerInner, target: Target) {
        if let Some(running) = inner.tasks.remove(&target.id) {
            running.handle.abort();
        }
        if target.enabled() {
            let id = target.id.clone();
            let handle = self.spawn_target(target.clone());
            inner.tasks.insert(id, RunningTarget { target, handle });
        }
    }

    pub async fn current_targets(&self) -> Vec<Target> {
        self.inner
            .lock()
            .await
            .tasks
            .values()
            .map(|running| running.target.clone())
            .collect()
    }

    pub async fn target(&self, id: &str) -> Option<Target> {
        self.inner
            .lock()
            .await
            .tasks
            .get(id)
            .map(|running| running.target.clone())
    }

    fn spawn_target(&self, target: Target) -> tokio::task::JoinHandle<()> {
        let config = self.config.clone();
        let db = self.db.clone();
        let client = self.client.clone();
        tokio::spawn(async move {
            if let Err(error) = run_target_loop(config, db, client, target).await {
                error!(%error, "target loop stopped");
            }
        })
    }
}

async fn run_target_loop(
    config: Arc<AppConfig>,
    db: Arc<dyn Persistence>,
    client: reqwest::Client,
    target: Target,
) -> Result<()> {
    let interval_secs = target.interval_secs(&config);
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
    let target_id = target.id.clone();
    match monitor::run_check(config, db, client, target).await {
        Ok(monitor::CheckReport::Checked {
            outcome,
            should_alert,
        }) => {
            info!(
                target_id = %outcome.target.id,
                matched = outcome.matched,
                engine = ?outcome.engine_used,
                price_cents = ?outcome.price_cents,
                should_alert,
                "check succeeded"
            );
            if should_alert {
                deliver_alert(config, db, client, &outcome).await;
            }
        }
        Ok(monitor::CheckReport::Failed { error }) => {
            warn!(error = %error, target_id = %target_id, "check failed");
        }
        Err(error) => {
            error!(%error, target_id = %target_id, "failed to record check");
        }
    }
}

async fn deliver_alert(
    config: &AppConfig,
    db: &dyn Persistence,
    client: &reqwest::Client,
    outcome: &CheckOutcome,
) {
    match discord::send_condition_alert(client, config, outcome).await {
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
