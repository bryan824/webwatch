use chrono::Utc;
use webwatch::{
    config::{CheckOutcome, ConditionKind, EngineUsed},
    config::{ConditionConfig, TargetConfig},
    db,
};

fn target_config() -> TargetConfig {
    TargetConfig {
        id: "target".to_string(),
        name: "Target".to_string(),
        url: "https://example.com/product".to_string(),
        enabled: true,
        interval_secs: None,
        conditions: vec![ConditionConfig {
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
        .sync_targets(std::slice::from_ref(&target_config))
        .await
        .expect("sync target");
    let target = target_config.to_target().expect("target");
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
async fn sync_targets_purges_removed_rows() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir
        .path()
        .join(format!("purge-{}.sqlite3", db::backend_name()));
    let persistence = db::connect(path.to_str().expect("utf8 path"))
        .await
        .expect("connect");
    persistence.migrate().await.expect("migrate");

    let target_a = target_config();
    let mut target_b = target_config();
    target_b.id = "removed".to_string();
    target_b.name = "Removed".to_string();

    persistence
        .sync_targets(&[target_a.clone(), target_b])
        .await
        .expect("initial sync");
    assert_eq!(persistence.statuses().await.expect("statuses").len(), 2);

    persistence
        .sync_targets(&[target_a])
        .await
        .expect("purge sync");
    let statuses = persistence.statuses().await.expect("statuses");

    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].target_id, "target");
    assert!(persistence
        .status("removed")
        .await
        .expect("status")
        .is_none());
}
