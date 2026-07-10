use std::{net::SocketAddr, sync::Arc};

use axum::{response::Html, routing::get, Router};
use reqwest::StatusCode;
use serde::Deserialize;
use webwatch::{
    config::{
        AppConfig, BrowserConfig, Condition, ConditionRule, RenderPlan, RendererConfig,
        SchedulerConfig, ServerConfig, Target, TargetStatus,
    },
    db,
    http::HttpState,
    scheduler::Scheduler,
};

#[derive(Debug, Deserialize)]
struct ImportResponse {
    added: Vec<String>,
    #[allow(dead_code)]
    removed: Vec<String>,
    changed: Vec<String>,
    #[allow(dead_code)]
    unchanged: Vec<String>,
}

async fn spawn_page_fixture() -> SocketAddr {
    let app = Router::new()
        .route("/a", get(page_a))
        .route("/b", get(page_b))
        .route("/c", get(page_c));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind fixture");
    let addr = listener.local_addr().expect("fixture addr");
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve fixture");
    });
    addr
}

async fn page_a() -> Html<&'static str> {
    Html("<html><body>Add to cart</body></html>")
}

async fn page_b() -> Html<&'static str> {
    Html("<html><body>Add to cart</body></html>")
}

async fn page_c() -> Html<&'static str> {
    Html("<html><body>Add to cart</body></html>")
}

fn target(id: &str, url: String, value: &str) -> Target {
    Target {
        id: id.to_string(),
        name: id.to_string(),
        url,
        enabled: true,
        interval_secs: Some(3_600),
        render: RenderPlan::default(),
        conditions: vec![Condition {
            id: Some("text".to_string()),
            rule: ConditionRule::Text {
                value: value.to_string(),
                negate: false,
            },
        }],
    }
}

fn write_targets(path: &std::path::Path, targets: &[Target]) {
    let body = targets
        .iter()
        .map(|target| {
            format!(
                r#"[[targets]]
id = "{}"
name = "{}"
url = "{}"
enabled = true
interval_secs = 3600

[[targets.conditions]]
id = "text"
kind = "text_appears"
value = "{}"
"#,
                target.id,
                target.name,
                target.url,
                target.conditions[0].value().unwrap_or("Add to cart")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(path, body).expect("write targets");
}

async fn spawn_webwatch(
    targets_path: std::path::PathBuf,
    targets: Vec<Target>,
) -> (SocketAddr, Arc<dyn db::Persistence>) {
    let dir = tempfile::tempdir().expect("tempdir").keep();
    let db_path = dir.join(format!("{}.sqlite3", db::backend_name()));
    let config = Arc::new(AppConfig {
        sqlite_path: db_path.to_string_lossy().to_string(),
        user_agent: "webwatch-test".to_string(),
        discord_webhook_url: None,
        targets_path: Some(targets_path.to_string_lossy().to_string()),
        server: ServerConfig::default(),
        scheduler: SchedulerConfig::default(),
        browser: BrowserConfig::default(),
        renderer: RendererConfig::default(),
    });
    let persistence: Arc<dyn db::Persistence> =
        Arc::from(db::connect(&config.sqlite_path).await.expect("connect"));
    persistence.migrate().await.expect("migrate");
    persistence.import_targets(&targets).await.expect("import");
    let client = reqwest::Client::new();
    let scheduler = Arc::new(Scheduler::new(
        config.clone(),
        persistence.clone(),
        client.clone(),
    ));
    scheduler.start(&targets).await;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let state = HttpState {
        config,
        scheduler,
        db: persistence.clone(),
        client,
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind webwatch");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        axum::serve(listener, webwatch::http::router(state))
            .await
            .expect("serve webwatch");
    });
    (addr, persistence)
}

#[tokio::test]
async fn import_targets_via_api_adds_and_persists() {
    let pages = spawn_page_fixture().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let targets_path = dir.path().join("targets.toml");
    let seed = target("A", format!("http://{pages}/a"), "Add to cart");
    write_targets(&targets_path, std::slice::from_ref(&seed));
    let (addr, _) = spawn_webwatch(targets_path, vec![seed]).await;

    let client = reqwest::Client::new();
    let body = format!(
        r#"[[targets]]
id = "imported"
name = "Imported"
url = "http://{pages}/b"
enabled = false
[[targets.conditions]]
kind = "text_appears"
value = "Add to cart"
"#
    );
    let response = client
        .post(format!("http://{addr}/targets/import"))
        .header("content-type", "application/toml")
        .body(body)
        .send()
        .await
        .expect("import");
    let status = response.status();
    let text = response.text().await.expect("body");
    assert_eq!(status, StatusCode::OK, "{text}");
    let report: ImportResponse = serde_json::from_str(&text).expect("json");
    assert_eq!(report.added, vec!["imported"]);

    let ids = client
        .get(format!("http://{addr}/targets"))
        .send()
        .await
        .expect("targets")
        .json::<Vec<TargetStatus>>()
        .await
        .expect("json")
        .into_iter()
        .map(|status| status.target_id)
        .collect::<Vec<_>>();
    assert_eq!(ids, vec!["A", "imported"]);
}

#[tokio::test]
async fn import_rejects_invalid_toml_with_bad_request() {
    let pages = spawn_page_fixture().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let targets_path = dir.path().join("targets.toml");
    let seed = target("A", format!("http://{pages}/a"), "Add to cart");
    write_targets(&targets_path, std::slice::from_ref(&seed));
    let (addr, _) = spawn_webwatch(targets_path, vec![seed]).await;

    let response = reqwest::Client::new()
        .post(format!("http://{addr}/targets/import"))
        .header("content-type", "application/toml")
        .body("[[targets]]\nname = \"no id\"\n")
        .send()
        .await
        .expect("import");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response.text().await.expect("body");
    assert!(body.contains("missing field `id`"), "{body}");
}

#[tokio::test]
async fn export_then_import_round_trips_over_http() {
    let pages = spawn_page_fixture().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let targets_path = dir.path().join("targets.toml");
    let target_a = target("A", format!("http://{pages}/a"), "Add to cart");
    let target_b = target("B", format!("http://{pages}/b"), "Add to cart");
    write_targets(&targets_path, &[target_a.clone(), target_b.clone()]);
    let (addr, _) = spawn_webwatch(targets_path, vec![target_a, target_b]).await;

    let client = reqwest::Client::new();
    let exported = client
        .get(format!("http://{addr}/targets/export"))
        .send()
        .await
        .expect("export");
    assert_eq!(exported.status(), StatusCode::OK);
    assert!(exported
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.contains("toml")));
    let toml = exported.text().await.expect("export body");
    assert!(toml.contains("id = \"A\""), "{toml}");
    assert!(toml.contains("id = \"B\""), "{toml}");

    // Re-importing the exported document is a no-op change set (both ids exist).
    let report: ImportResponse = client
        .post(format!("http://{addr}/targets/import"))
        .header("content-type", "application/toml")
        .body(toml)
        .send()
        .await
        .expect("reimport")
        .json()
        .await
        .expect("json");
    assert!(report.added.is_empty());
    assert_eq!(report.changed, vec!["A", "B"]);
}

#[tokio::test]
async fn create_target_via_api() {
    let pages = spawn_page_fixture().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let targets_path = dir.path().join("targets.toml");
    let seed = target("A", format!("http://{pages}/a"), "Add to cart");
    write_targets(&targets_path, std::slice::from_ref(&seed));
    let (addr, _) = spawn_webwatch(targets_path, vec![seed]).await;

    let client = reqwest::Client::new();
    let created = client
        .post(format!("http://{addr}/targets"))
        .json(&serde_json::json!({
            "name": "Campfire Mug",
            "url": format!("http://{pages}/b"),
            "render": {
                "policy": "render_first",
                "fingerprint_seed": "campfire-mug",
                "wait_ms": 3000
            },
            "conditions": [{"kind": "text_appears", "value": "Add to cart"}],
        }))
        .send()
        .await
        .expect("create");
    assert_eq!(created.status(), StatusCode::CREATED);
    let status = created.json::<TargetStatus>().await.expect("json");
    assert_eq!(status.target_id, "campfire-mug");
    assert_eq!(
        status.render.policy,
        webwatch::config::RenderPolicy::RenderFirst
    );
    assert_eq!(
        status.render.fingerprint_seed.as_deref(),
        Some("campfire-mug")
    );

    let response = client
        .get(format!("http://{addr}/targets"))
        .send()
        .await
        .expect("targets");
    let status_code = response.status();
    let body = response.text().await.expect("body");
    assert_eq!(status_code, StatusCode::OK, "{body}");
    let ids = serde_json::from_str::<Vec<TargetStatus>>(&body)
        .expect("json")
        .into_iter()
        .map(|status| status.target_id)
        .collect::<Vec<_>>();
    assert!(ids.contains(&"campfire-mug".to_string()));
}

#[tokio::test]
async fn create_target_rejects_missing_required_condition_field() {
    let pages = spawn_page_fixture().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let targets_path = dir.path().join("targets.toml");
    let seed = target("A", format!("http://{pages}/a"), "Add to cart");
    write_targets(&targets_path, std::slice::from_ref(&seed));
    let (addr, _) = spawn_webwatch(targets_path, vec![seed]).await;

    let response = reqwest::Client::new()
        .post(format!("http://{addr}/targets"))
        .json(&serde_json::json!({
            "name": "Invalid Target",
            "url": format!("http://{pages}/b"),
            "conditions": [{"kind": "text_appears"}],
        }))
        .send()
        .await
        .expect("create");

    let status = response.status();
    let body = response.text().await.expect("body");

    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(
        body.contains("condition condition-1 requires value"),
        "{body}"
    );
}

#[tokio::test]
async fn delete_target_via_api() {
    let pages = spawn_page_fixture().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let targets_path = dir.path().join("targets.toml");
    let target_a = target("A", format!("http://{pages}/a"), "Add to cart");
    let target_b = target("B", format!("http://{pages}/b"), "Add to cart");
    write_targets(&targets_path, &[target_a.clone(), target_b.clone()]);
    let (addr, _) = spawn_webwatch(targets_path, vec![target_a, target_b]).await;

    let client = reqwest::Client::new();
    let response = client
        .delete(format!("http://{addr}/targets/B"))
        .send()
        .await
        .expect("delete");
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let ids = client
        .get(format!("http://{addr}/targets"))
        .send()
        .await
        .expect("targets")
        .json::<Vec<TargetStatus>>()
        .await
        .expect("json")
        .into_iter()
        .map(|status| status.target_id)
        .collect::<Vec<_>>();
    assert_eq!(ids, vec!["A"]);
}

#[tokio::test]
async fn toggle_enabled_via_api() {
    let pages = spawn_page_fixture().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let targets_path = dir.path().join("targets.toml");
    let target_a = target("A", format!("http://{pages}/a"), "Add to cart");
    write_targets(&targets_path, std::slice::from_ref(&target_a));
    let (addr, _) = spawn_webwatch(targets_path, vec![target_a]).await;

    let client = reqwest::Client::new();
    let response = client
        .patch(format!("http://{addr}/targets/A"))
        .json(&serde_json::json!({ "enabled": false }))
        .send()
        .await
        .expect("patch");
    assert_eq!(response.status(), StatusCode::OK);

    // Disabled, not deleted — still listed.
    let ids = client
        .get(format!("http://{addr}/targets"))
        .send()
        .await
        .expect("targets")
        .json::<Vec<TargetStatus>>()
        .await
        .expect("json")
        .into_iter()
        .map(|status| status.target_id)
        .collect::<Vec<_>>();
    assert_eq!(ids, vec!["A"]);
}

#[tokio::test]
async fn get_target_detail_returns_persisted_config() {
    let pages = spawn_page_fixture().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let targets_path = dir.path().join("targets.toml");
    let target_a = target("A", format!("http://{pages}/a"), "Add to cart");
    write_targets(&targets_path, std::slice::from_ref(&target_a));
    let (addr, _) = spawn_webwatch(targets_path, vec![target_a]).await;

    let body = reqwest::Client::new()
        .get(format!("http://{addr}/targets/A"))
        .send()
        .await
        .expect("detail")
        .text()
        .await
        .expect("body");
    let json: serde_json::Value = serde_json::from_str(&body).expect("json");

    assert_eq!(json["config"]["id"], "A");
    assert_eq!(json["config"]["conditions"][0]["kind"], "text_appears");
    assert_eq!(json["status"]["target_id"], "A");
}

#[tokio::test]
async fn update_target_via_api_preserves_id_without_duplicate() {
    let pages = spawn_page_fixture().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let targets_path = dir.path().join("targets.toml");
    let target_a = target("A", format!("http://{pages}/a"), "Add to cart");
    write_targets(&targets_path, std::slice::from_ref(&target_a));
    let (addr, _) = spawn_webwatch(targets_path, vec![target_a]).await;

    let client = reqwest::Client::new();
    let response = client
        .put(format!("http://{addr}/targets/A"))
        .json(&serde_json::json!({
            "name": "Updated Watch",
            "url": format!("http://{pages}/b"),
            "enabled": true,
            "interval_secs": 3600,
            "conditions": [{"id": "text", "kind": "text_appears", "value": "Add to cart"}],
        }))
        .send()
        .await
        .expect("put");
    assert_eq!(response.status(), StatusCode::OK);

    let statuses = client
        .get(format!("http://{addr}/targets"))
        .send()
        .await
        .expect("targets")
        .json::<Vec<TargetStatus>>()
        .await
        .expect("json");

    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].target_id, "A");
    assert_eq!(statuses[0].name, "Updated Watch");
    assert_eq!(statuses[0].url, format!("http://{pages}/b"));
}

#[tokio::test]
async fn dry_run_returns_evidence_without_saving_target() {
    let pages = spawn_page_fixture().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let targets_path = dir.path().join("targets.toml");
    let seed = target("A", format!("http://{pages}/a"), "Add to cart");
    write_targets(&targets_path, std::slice::from_ref(&seed));
    let (addr, _) = spawn_webwatch(targets_path, vec![seed]).await;

    let client = reqwest::Client::new();
    let response = client
        .post(format!("http://{addr}/targets/dry-run"))
        .json(&serde_json::json!({
            "name": "Draft Watch",
            "url": format!("http://{pages}/b"),
            "conditions": [{"kind": "text_appears", "value": "Add to cart"}],
        }))
        .send()
        .await
        .expect("dry run");
    assert_eq!(response.status(), StatusCode::OK);
    let json: serde_json::Value = response.json().await.expect("json");
    assert_eq!(json["matched"], true);
    assert_eq!(json["engine_used"], "http");
    assert!(json["evidence"][0]
        .as_str()
        .unwrap()
        .contains("Add to cart"));

    let ids = client
        .get(format!("http://{addr}/targets"))
        .send()
        .await
        .expect("targets")
        .json::<Vec<TargetStatus>>()
        .await
        .expect("json")
        .into_iter()
        .map(|status| status.target_id)
        .collect::<Vec<_>>();
    assert_eq!(ids, vec!["A"]);
}

#[tokio::test]
async fn ops_status_reports_counts_and_renderer_state() {
    let pages = spawn_page_fixture().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let targets_path = dir.path().join("targets.toml");
    let target_a = target("A", format!("http://{pages}/a"), "Add to cart");
    write_targets(&targets_path, std::slice::from_ref(&target_a));
    let (addr, _) = spawn_webwatch(targets_path, vec![target_a]).await;

    let json = reqwest::Client::new()
        .get(format!("http://{addr}/ops"))
        .send()
        .await
        .expect("ops")
        .json::<serde_json::Value>()
        .await
        .expect("json");

    assert_eq!(json["status"], "ok");
    assert_eq!(json["targets"]["total"], 1);
    assert_eq!(json["renderer_available"], false);
}

#[tokio::test]
async fn recent_checks_endpoint_returns_recorded_runs() {
    let pages = spawn_page_fixture().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let targets_path = dir.path().join("targets.toml");
    let target_a = target("A", format!("http://{pages}/a"), "Add to cart");
    write_targets(&targets_path, std::slice::from_ref(&target_a));
    let (addr, _) = spawn_webwatch(targets_path, vec![target_a]).await;

    let client = reqwest::Client::new();
    let checked = client
        .get(format!("http://{addr}/targets/A/status"))
        .send()
        .await
        .expect("check");
    assert_eq!(checked.status(), StatusCode::OK);

    let runs = client
        .get(format!("http://{addr}/targets/A/checks"))
        .send()
        .await
        .expect("checks")
        .json::<Vec<serde_json::Value>>()
        .await
        .expect("json");

    assert!(!runs.is_empty());
    assert_eq!(runs[0]["matched"], true);
}
