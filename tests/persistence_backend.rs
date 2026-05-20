use chrono::Utc;
use webwatch::{
    config::{ConditionConfig, TargetConfig},
    db,
    models::{CheckOutcome, ConditionKind, EngineUsed},
};

fn target_config() -> TargetConfig {
    TargetConfig {
        id: "target".to_string(),
        name: "Target".to_string(),
        url: "https://example.com/product".to_string(),
        enabled: Some(true),
        interval_secs: None,
        conditions: vec![ConditionConfig {
            id: Some("stock".to_string()),
            kind: ConditionKind::TextAppears,
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
        .ensure_target(&target_config)
        .await
        .expect("ensure target");
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
