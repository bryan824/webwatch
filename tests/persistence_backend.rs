use chrono::Utc;
use webwatch::{
    config::{CheckOutcome, ConditionKind, EngineUsed},
    config::{Condition, Target},
    db,
};

fn target_config() -> Target {
    Target {
        id: "target".to_string(),
        name: "Target".to_string(),
        url: "https://example.com/product".to_string(),
        enabled: true,
        interval_secs: None,
        conditions: vec![Condition {
            id: Some("stock".to_string()),
            kind: ConditionKind::Text,
            negate: false,
            value: Some("Add to cart".to_string()),
            selector: None,
            threshold_cents: None,
            price_selector: None,
        }],
    }
}

#[tokio::test]
async fn active_backend_persists_status() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join(format!("{}.sqlite3", db::backend_name()));
    let persistence = db::connect(path.to_str().expect("utf8 path"))
        .await
        .expect("connect");
    persistence.migrate().await.expect("migrate");

    let target_config = target_config();
    persistence
        .import_targets(std::slice::from_ref(&target_config))
        .await
        .expect("import target");
    let target = target_config.validated().expect("valid target");
    let outcome = CheckOutcome {
        target,
        engine_used: EngineUsed::Http,
        matched: true,
        checked_at: Utc::now(),
        price_cents: Some(4_250),
        evidence: vec!["page text contains 'Add to cart'".to_string()],
        condition_results: vec![],
    };

    assert!(persistence
        .record_success(&outcome)
        .await
        .expect("record success"));
    let status = persistence
        .status("target")
        .await
        .expect("status")
        .expect("target status");

    assert_eq!(status.matched, Some(true));
    assert_eq!(status.price_cents, Some(4_250));
    assert_eq!(status.engine_used, Some(EngineUsed::Http));
}

#[tokio::test]
async fn status_returns_existing_target_without_checks() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir
        .path()
        .join(format!("empty-status-{}.sqlite3", db::backend_name()));
    let persistence = db::connect(path.to_str().expect("utf8 path"))
        .await
        .expect("connect");
    persistence.migrate().await.expect("migrate");

    persistence
        .import_targets(std::slice::from_ref(&target_config()))
        .await
        .expect("import target");

    let status = persistence
        .status("target")
        .await
        .expect("status")
        .expect("target status");

    assert_eq!(status.target_id, "target");
    assert_eq!(status.name, "Target");
    assert_eq!(status.url, "https://example.com/product");
    assert!(status.enabled);
    assert_eq!(status.matched, None);
    assert_eq!(status.engine_used, None);
    assert_eq!(status.price_cents, None);
    assert!(status.evidence.is_empty());
    assert!(status.condition_results.is_empty());
    assert_eq!(status.last_success_at, None);
    assert_eq!(status.last_error_at, None);
    assert_eq!(status.last_error, None);
    assert_eq!(status.last_alert_at, None);
}

#[tokio::test]
async fn status_returns_none_for_missing_target() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir
        .path()
        .join(format!("missing-status-{}.sqlite3", db::backend_name()));
    let persistence = db::connect(path.to_str().expect("utf8 path"))
        .await
        .expect("connect");
    persistence.migrate().await.expect("migrate");
    persistence
        .import_targets(std::slice::from_ref(&target_config()))
        .await
        .expect("import target");

    assert!(persistence
        .status("missing")
        .await
        .expect("status")
        .is_none());
}

#[tokio::test]
async fn import_targets_upserts_without_purging() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir
        .path()
        .join(format!("import-{}.sqlite3", db::backend_name()));
    let persistence = db::connect(path.to_str().expect("utf8 path"))
        .await
        .expect("connect");
    persistence.migrate().await.expect("migrate");

    let target_a = target_config();
    let mut target_b = target_config();
    target_b.id = "second".to_string();
    target_b.name = "Second".to_string();

    persistence
        .import_targets(&[target_a.clone(), target_b])
        .await
        .expect("initial import");
    assert_eq!(persistence.list_targets().await.expect("list").len(), 2);

    // Re-importing only A must NOT purge B (the DB is authoritative).
    persistence
        .import_targets(std::slice::from_ref(&target_a))
        .await
        .expect("re-import");
    assert_eq!(persistence.list_targets().await.expect("list").len(), 2);

    // Explicit removal drops the row and its state.
    persistence.remove_target("second").await.expect("remove");
    let remaining = persistence.list_targets().await.expect("list");
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].id, "target");
    assert!(persistence
        .status("second")
        .await
        .expect("status")
        .is_none());
}

#[tokio::test]
async fn set_enabled_persists_flag() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir
        .path()
        .join(format!("enabled-{}.sqlite3", db::backend_name()));
    let persistence = db::connect(path.to_str().expect("utf8 path"))
        .await
        .expect("connect");
    persistence.migrate().await.expect("migrate");
    persistence
        .import_targets(std::slice::from_ref(&target_config()))
        .await
        .expect("import");

    persistence
        .set_enabled("target", false)
        .await
        .expect("disable");
    let target = persistence
        .list_targets()
        .await
        .expect("list")
        .into_iter()
        .find(|target| target.id == "target")
        .expect("target");
    assert!(!target.enabled);
}
