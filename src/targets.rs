use std::sync::Arc;

use serde::Serialize;
use snafu::ResultExt;
use tracing::info;

use crate::{
    check,
    config::{AppConfig, CheckOutcome, Condition, RenderPlan, Target, TargetStatus, TargetsFile},
    db::Persistence,
    error::{ParseConfigSnafu, SerializeTargetsSnafu},
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
    pub render: RenderPlan,
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
            render: command.render,
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

    pub async fn config(&self, id: &str) -> Result<Option<Target>> {
        Ok(self
            .db
            .list_targets()
            .await?
            .into_iter()
            .find(|target| target.id == id))
    }

    pub async fn update(&self, id: &str, command: CreateTarget) -> Result<Option<TargetStatus>> {
        let exists = self
            .db
            .list_targets()
            .await?
            .into_iter()
            .any(|target| target.id == id);
        if !exists {
            return Ok(None);
        }

        let target = Target {
            id: id.to_string(),
            name: command.name,
            url: command.url,
            enabled: command.enabled.unwrap_or(true),
            interval_secs: command.interval_secs,
            render: command.render,
            conditions: command.conditions,
        }
        .validated()?;

        self.db.ensure_target(&target).await?;
        self.scheduler.reconcile_target(target).await;
        self.db.status(id).await
    }

    pub async fn dry_run(
        &self,
        config: &AppConfig,
        client: &reqwest::Client,
        target: Target,
    ) -> Result<CheckOutcome> {
        let renderer = self.scheduler.renderer();
        check::check_target(config, client, &renderer, target.validated()?).await
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

    /// Serialize all stored targets back into `targets.toml` interchange format.
    /// Round-trips with [`import_from_toml`]: exporting then re-importing yields
    /// identical state, so the DB stays the single source of truth while config
    /// can still be snapshotted out to a file (or copied to another instance).
    pub async fn export_toml(&self) -> Result<String> {
        let targets = self.db.list_targets().await?;
        let file = TargetsFile { targets };
        toml::to_string_pretty(&file).context(SerializeTargetsSnafu)
    }

    /// Import targets from pasted TOML in the same `[[targets]]` shape as
    /// `targets.toml`, so a watch can be added by pasting a config snippet
    /// instead of filling out the builder form. Unlike [`reload`], this reports
    /// and persists every pasted target regardless of its `enabled` flag (an
    /// existing id is counted as `changed`, a new id as `added`), and reconciles
    /// each with the scheduler so disabling via paste also takes effect.
    pub async fn import_from_toml(&self, raw: &str) -> Result<ReloadReport> {
        let parsed: TargetsFile = toml::from_str(raw).context(ParseConfigSnafu {
            path: "<pasted targets>".to_string(),
        })?;
        let parsed = parsed.resolve_and_validate()?;

        let existing_ids = self
            .db
            .list_targets()
            .await?
            .into_iter()
            .map(|target| target.id)
            .collect::<Vec<_>>();

        let mut report = ReloadReport::default();
        for target in &parsed.targets {
            if existing_ids.iter().any(|id| id == &target.id) {
                report.changed.push(target.id.clone());
            } else {
                report.added.push(target.id.clone());
            }
        }

        self.db.import_targets(&parsed.targets).await?;
        for target in parsed.targets {
            self.scheduler.reconcile_target(target).await;
        }

        sort_report(&mut report);
        info!(added = ?report.added, changed = ?report.changed, "imported targets from pasted TOML");
        Ok(report)
    }

    pub async fn check_target_by_id(
        &self,
        config: &AppConfig,
        client: &reqwest::Client,
        id: &str,
        mark_manual_report: bool,
    ) -> Result<bool> {
        let target = if let Some(target) = self.scheduler.target(id).await {
            target
        } else {
            let Some(target) = self
                .db
                .list_targets()
                .await?
                .into_iter()
                .find(|target| target.id == id)
            else {
                return Ok(false);
            };
            target
        };

        let renderer = self.scheduler.renderer();
        match monitor::run_check(config, self.db.as_ref(), client, &renderer, target).await? {
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
            AppConfig, BrowserConfig, Condition, ConditionRule, RenderPlan, RendererConfig,
            SchedulerConfig, ServerConfig, Target,
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
            renderer: RendererConfig::default(),
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
            render: RenderPlan::default(),
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
            render: RenderPlan::default(),
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
                render: RenderPlan::default(),
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

    #[tokio::test]
    async fn import_from_toml_reports_added_and_changed_regardless_of_enabled() {
        let (lifecycle, db, _dir) = test_lifecycle().await;
        db.ensure_target(&seed_target("existing-watch"))
            .await
            .expect("seed");

        // A brand-new disabled target plus an update to the existing one. The
        // disabled target must still be reported as `added` and persisted.
        let report = lifecycle
            .import_from_toml(
                r#"
                [[targets]]
                id = "fresh-watch"
                name = "Fresh"
                url = "https://example.com/fresh"
                enabled = false
                [[targets.conditions]]
                kind = "url_unchanged"

                [[targets]]
                id = "existing-watch"
                name = "Existing (updated)"
                url = "https://example.com/updated"
                [[targets.conditions]]
                kind = "text_appears"
                value = "In stock"
                "#,
            )
            .await
            .expect("import");

        assert_eq!(report.added, vec!["fresh-watch".to_string()]);
        assert_eq!(report.changed, vec!["existing-watch".to_string()]);

        let ids = db
            .list_targets()
            .await
            .expect("list")
            .into_iter()
            .map(|target| target.id)
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["existing-watch", "fresh-watch"]);
    }

    #[tokio::test]
    async fn import_from_toml_rejects_invalid_toml_without_persisting() {
        let (lifecycle, db, _dir) = test_lifecycle().await;

        let error = lifecycle
            .import_from_toml("[[targets]]\nname = \"missing id\"\n")
            .await
            .expect_err("invalid toml");

        assert!(matches!(error, crate::Error::ParseConfig { .. }));
        assert!(db.list_targets().await.expect("list").is_empty());
    }

    #[tokio::test]
    async fn export_round_trips_through_import() {
        // Targets kept disabled so re-import doesn't spawn live check loops.
        // Covers every optional-field shape: a bare redirect condition, a
        // selector+value condition, a price condition, and a non-default render
        // plan — exactly what would break a naive TOML serializer.
        let source = r#"
            [[targets]]
            id = "alpha"
            name = "Alpha"
            url = "https://example.com/alpha"
            enabled = false
            interval_secs = 600
            [[targets.conditions]]
            id = "in-stock"
            kind = "url_unchanged"

            [[targets]]
            id = "beta"
            name = "Beta"
            url = "https://example.com/beta"
            enabled = false
            [targets.render]
            policy = "render_first"
            [[targets.conditions]]
            id = "atb"
            kind = "selector_text_contains"
            selector = ".btn"
            value = "Add to cart"
            [[targets.conditions]]
            id = "cheap"
            kind = "price_below"
            threshold_cents = 5000
            price_selector = ".price"
        "#;

        let (lifecycle_a, db_a, _dir_a) = test_lifecycle().await;
        lifecycle_a.import_from_toml(source).await.expect("seed");
        let exported = lifecycle_a.export_toml().await.expect("export");

        let (lifecycle_b, db_b, _dir_b) = test_lifecycle().await;
        lifecycle_b
            .import_from_toml(&exported)
            .await
            .expect("re-import exported toml");

        let a = db_a.list_targets().await.expect("list a");
        let b = db_b.list_targets().await.expect("list b");
        assert_eq!(a.len(), 2);
        assert_eq!(a, b, "export -> import must be identity\n--- exported ---\n{exported}");
    }
}
