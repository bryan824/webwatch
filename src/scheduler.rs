use std::{collections::HashMap, sync::Arc, time::Duration};

use rand::Rng;
use serde::Serialize;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use crate::{config::AppConfig, config::Target, db::Persistence, discord, evaluator, Result};

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

#[derive(Debug, Clone, Serialize, Default)]
pub struct ReloadReport {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub changed: Vec<String>,
    pub unchanged: Vec<String>,
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

    pub async fn reload(&self, targets: &[Target]) -> Result<ReloadReport> {
        let mut inner = self.inner.lock().await;
        let (mut report, plan) = diff_targets(&inner.tasks, targets);
        self.db.sync_targets(targets).await?;
        apply_plan(&mut inner, plan, self);
        sort_report(&mut report);
        info!(
            added = ?report.added,
            removed = ?report.removed,
            changed = ?report.changed,
            unchanged = ?report.unchanged,
            "scheduler reload"
        );
        Ok(report)
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

enum PlanStep {
    Remove(String),
    Add(Target),
    Change(Target),
}

fn diff_targets(
    current: &HashMap<String, RunningTarget>,
    targets: &[Target],
) -> (ReloadReport, Vec<PlanStep>) {
    let desired = targets
        .iter()
        .filter(|target| target.enabled())
        .map(|target| (target.id.clone(), target.clone()))
        .collect::<HashMap<_, _>>();
    let mut report = ReloadReport::default();
    let mut plan = Vec::new();

    for (id, running) in current {
        match desired.get(id) {
            Some(target) if *target == running.target => report.unchanged.push(id.clone()),
            Some(target) => {
                report.changed.push(id.clone());
                plan.push(PlanStep::Change(target.clone()));
            }
            None => {
                report.removed.push(id.clone());
                plan.push(PlanStep::Remove(id.clone()));
            }
        }
    }

    for (id, target) in desired {
        if !current.contains_key(&id) {
            report.added.push(id);
            plan.push(PlanStep::Add(target));
        }
    }

    (report, plan)
}

fn apply_plan(inner: &mut SchedulerInner, plan: Vec<PlanStep>, scheduler: &Scheduler) {
    for step in plan {
        match step {
            PlanStep::Remove(id) => {
                if let Some(running) = inner.tasks.remove(&id) {
                    running.handle.abort();
                }
            }
            PlanStep::Add(target) => {
                let id = target.id.clone();
                let handle = scheduler.spawn_target(target.clone());
                inner.tasks.insert(id, RunningTarget { target, handle });
            }
            PlanStep::Change(target) => {
                if let Some(running) = inner.tasks.remove(&target.id) {
                    running.handle.abort();
                }
                let id = target.id.clone();
                let handle = scheduler.spawn_target(target.clone());
                inner.tasks.insert(id, RunningTarget { target, handle });
            }
        }
    }
}

fn sort_report(report: &mut ReloadReport) {
    report.added.sort();
    report.removed.sort();
    report.changed.sort();
    report.unchanged.sort();
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
