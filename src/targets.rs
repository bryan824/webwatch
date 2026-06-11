use std::{path::Path as FsPath, sync::Arc};

use serde::Serialize;
use tracing::info;

use crate::{
    config::{AppConfig, Condition, Target, TargetStatus, TargetsFile},
    db::Persistence,
    monitor,
    scheduler::Scheduler,
    Error, Result,
};

#[derive(Debug, Clone)]
pub struct CreateTarget {
    pub name: String,
    pub url: String,
    pub enabled: Option<bool>,
    pub interval_secs: Option<u64>,
    pub conditions: Vec<Condition>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ReloadReport {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub changed: Vec<String>,
    pub unchanged: Vec<String>,
}

#[derive(Clone)]
pub struct TargetLifecycle {
    db: Arc<dyn Persistence>,
    scheduler: Arc<Scheduler>,
}

impl TargetLifecycle {
    pub fn new(db: Arc<dyn Persistence>, scheduler: Arc<Scheduler>) -> Self {
        Self { db, scheduler }
    }

    pub async fn create(&self, command: CreateTarget) -> Result<TargetStatus> {
        let existing_ids = self
            .db
            .list_targets()
            .await?
            .into_iter()
            .map(|target| target.id)
            .collect::<Vec<_>>();

        let target = Target {
            id: unique_slug(&command.name, &existing_ids),
            name: command.name,
            url: command.url,
            enabled: command.enabled.unwrap_or(true),
            interval_secs: command.interval_secs,
            conditions: command.conditions,
        }
        .validated()?;

        let id = target.id.clone();
        self.db.ensure_target(&target).await?;
        self.scheduler.reconcile_target(target).await;
        self.db.status(&id).await?.ok_or_else(|| Error::Database {
            message: format!("created target {id} missing status"),
        })
    }

    pub async fn statuses(&self) -> Result<Vec<TargetStatus>> {
        self.db.statuses().await
    }

    pub async fn status(
        &self,
        config: &AppConfig,
        client: &reqwest::Client,
        id: &str,
    ) -> Result<Option<TargetStatus>> {
        if !self.check_target_by_id(config, client, id, false).await? {
            return Ok(None);
        }
        self.db.status(id).await
    }

    pub async fn delete(&self, id: &str) -> Result<bool> {
        let exists = self
            .db
            .list_targets()
            .await?
            .iter()
            .any(|target| target.id == id);
        if !exists {
            return Ok(false);
        }
        self.db.remove_target(id).await?;
        self.scheduler.remove_running_target(id).await;
        Ok(true)
    }

    pub async fn set_enabled(&self, id: &str, enabled: bool) -> Result<Option<TargetStatus>> {
        let Some(mut target) = self
            .db
            .list_targets()
            .await?
            .into_iter()
            .find(|target| target.id == id)
        else {
            return Ok(None);
        };
        self.db.set_enabled(id, enabled).await?;
        target.enabled = enabled;
        self.scheduler.reconcile_target(target).await;
        self.db.status(id).await
    }

    pub async fn reload_from_config(&self, config: &AppConfig) -> Result<ReloadReport> {
        let Some(path) = config.targets_path.as_deref() else {
            return Err(Error::Database {
                message: "targets_path not configured".to_string(),
            });
        };
        let targets = TargetsFile::load(FsPath::new(path))?;
        self.reload(&targets.targets).await
    }

    pub async fn reload(&self, targets: &[Target]) -> Result<ReloadReport> {
        self.db.import_targets(targets).await?;
        let mut report = ReloadReport::default();

        for target in targets.iter().filter(|target| target.enabled()).cloned() {
            match self
                .scheduler
                .target(&target.id)
                .await
                .map(|running| running == target)
            {
                Some(true) => report.unchanged.push(target.id.clone()),
                Some(false) => {
                    report.changed.push(target.id.clone());
                    self.scheduler.reconcile_target(target).await;
                }
                None => {
                    report.added.push(target.id.clone());
                    self.scheduler.reconcile_target(target).await;
                }
            }
        }

        sort_report(&mut report);
        info!(added = ?report.added, changed = ?report.changed, unchanged = ?report.unchanged, "target lifecycle reload/import");
        Ok(report)
    }

    pub async fn check_target_by_id(
        &self,
        config: &AppConfig,
        client: &reqwest::Client,
        id: &str,
        mark_manual_report: bool,
    ) -> Result<bool> {
        let Some(target) = self.scheduler.target(id).await else {
            return Ok(false);
        };

        match monitor::run_check(config, self.db.as_ref(), client, target).await? {
            monitor::CheckReport::Checked {
                outcome,
                should_alert,
            } => {
                if should_alert && mark_manual_report {
                    self.db.mark_alert_sent(&outcome.target.id).await?;
                }
                Ok(true)
            }
            monitor::CheckReport::Failed { .. } => Ok(true),
        }
    }
}

fn unique_slug(name: &str, existing: &[String]) -> String {
    let base = slugify(name);
    let base = if base.is_empty() {
        "target".to_string()
    } else {
        base
    };
    if !existing.iter().any(|id| id == &base) {
        return base;
    }
    (2..)
        .map(|suffix| format!("{base}-{suffix}"))
        .find(|candidate| !existing.iter().any(|id| id == candidate))
        .expect("slug suffix space is unbounded")
}

fn slugify(name: &str) -> String {
    name.to_lowercase()
        .split(|character: char| !character.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn sort_report(report: &mut ReloadReport) {
    report.added.sort();
    report.removed.sort();
    report.changed.sort();
    report.unchanged.sort();
}

#[cfg(test)]
mod tests {
    use super::{CreateTarget, TargetLifecycle};
    use crate::{
        config::{
            AppConfig, BrowserConfig, Condition, ConditionRule, SchedulerConfig, ServerConfig,
            Target,
        },
        db,
        scheduler::Scheduler,
    };
    use std::{path::PathBuf, sync::Arc};

    async fn test_lifecycle() -> (TargetLifecycle, Arc<dyn db::Persistence>, PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir").keep();
        let db_path = dir.join(format!("lifecycle-{}.sqlite3", db::backend_name()));
        let config = Arc::new(AppConfig {
            sqlite_path: db_path.to_string_lossy().to_string(),
            user_agent: "webwatch-test".to_string(),
            discord_webhook_url: None,
            targets_path: None,
            server: ServerConfig::default(),
            scheduler: SchedulerConfig::default(),
            browser: BrowserConfig::default(),
        });
        let persistence: Arc<dyn db::Persistence> =
            Arc::from(db::connect(&config.sqlite_path).await.expect("connect"));
        persistence.migrate().await.expect("migrate");
        let scheduler = Arc::new(Scheduler::new(
            config,
            persistence.clone(),
            reqwest::Client::new(),
        ));
        (
            TargetLifecycle::new(persistence.clone(), scheduler),
            persistence,
            dir,
        )
    }

    fn create_command(name: &str) -> CreateTarget {
        CreateTarget {
            name: name.to_string(),
            url: "https://example.com/product".to_string(),
            enabled: Some(false),
            interval_secs: None,
            conditions: vec![Condition {
                id: None,
                rule: ConditionRule::Text {
                    value: "Add to cart".to_string(),
                    negate: false,
                },
            }],
        }
    }

    fn seed_target(id: &str) -> Target {
        Target {
            id: id.to_string(),
            name: "Seed".to_string(),
            url: "https://example.com/seed".to_string(),
            enabled: false,
            interval_secs: None,
            conditions: vec![Condition {
                id: Some("condition-1".to_string()),
                rule: ConditionRule::Text {
                    value: "Add to cart".to_string(),
                    negate: false,
                },
            }],
        }
    }

    #[tokio::test]
    async fn create_generates_slugged_target_id() {
        let (lifecycle, db, _dir) = test_lifecycle().await;

        let status = lifecycle
            .create(create_command("Campfire Mug"))
            .await
            .expect("create target");

        assert_eq!(status.target_id, "campfire-mug");
        assert_eq!(status.name, "Campfire Mug");
        assert!(!status.enabled);
        assert!(db.status("campfire-mug").await.expect("status").is_some());
    }

    #[tokio::test]
    async fn create_uses_next_slug_on_collision() {
        let (lifecycle, db, _dir) = test_lifecycle().await;
        db.ensure_target(&seed_target("campfire-mug"))
            .await
            .expect("seed");

        let status = lifecycle
            .create(create_command("Campfire Mug"))
            .await
            .expect("create target");

        assert_eq!(status.target_id, "campfire-mug-2");
        let ids = db
            .list_targets()
            .await
            .expect("list")
            .into_iter()
            .map(|target| target.id)
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["campfire-mug", "campfire-mug-2"]);
    }

    #[tokio::test]
    async fn create_invalid_target_returns_error_without_inserting() {
        let (lifecycle, db, _dir) = test_lifecycle().await;

        let error = lifecycle
            .create(CreateTarget {
                name: "Invalid Target".to_string(),
                url: "https://example.com/product".to_string(),
                enabled: Some(false),
                interval_secs: None,
                conditions: vec![Condition {
                    id: None,
                    rule: ConditionRule::Invalid {
                        kind: crate::config::ConditionKind::Text,
                        negate: false,
                        missing_field: "value",
                    },
                }],
            })
            .await
            .expect_err("invalid target");

        assert!(error
            .to_string()
            .contains("condition condition-1 requires value"));
        assert!(db.list_targets().await.expect("list").is_empty());
        assert!(db.status("invalid-target").await.expect("status").is_none());
    }
}
