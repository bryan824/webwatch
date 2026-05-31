use std::{net::SocketAddr, sync::Arc};

use axum::{response::Html, routing::get, Router};
use reqwest::StatusCode;
use serde::Deserialize;
use webwatch::{
    config::{
        AppConfig, BrowserConfig, Condition, ConditionKind, SchedulerConfig, ServerConfig,
        Target, TargetStatus,
    },
    db,
    http::HttpState,
    scheduler::Scheduler,
};

#[derive(Debug, Deserialize)]
struct ReloadResponse {
    added: Vec<String>,
    removed: Vec<String>,
    changed: Vec<String>,
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
        conditions: vec![Condition {
            id: Some("text".to_string()),
            kind: ConditionKind::Text,
            negate: false,
            value: Some(value.to_string()),
            selector: None,
            threshold_cents: None,
            price_selector: None,
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
                target.conditions[0]
                    .value
                    .as_deref()
                    .unwrap_or("Add to cart")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(path, body).expect("write targets");
}

async fn reload(addr: SocketAddr) -> (StatusCode, String) {
    let response = reqwest::Client::new()
        .post(format!("http://{addr}/targets/reload"))
        .bearer_auth("secret")
        .send()
        .await
        .expect("reload");
    let status = response.status();
    let body = response.text().await.expect("body");
    (status, body)
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
        api_token: Some("secret".to_string()),
        targets_path: Some(targets_path.to_string_lossy().to_string()),
        server: ServerConfig::default(),
        scheduler: SchedulerConfig::default(),
        browser: BrowserConfig::default(),
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
async fn reload_same_targets_reports_unchanged() {
    let pages = spawn_page_fixture().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let targets_path = dir.path().join("targets.toml");
    let targets = vec![
        target("A", format!("http://{pages}/a"), "Add to cart"),
        target("B", format!("http://{pages}/b"), "Add to cart"),
    ];
    write_targets(&targets_path, &targets);
    let (addr, _) = spawn_webwatch(targets_path, targets).await;

    let (status, body) = reload(addr).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let response: ReloadResponse = serde_json::from_str(&body).expect("json");

    assert_eq!(response.unchanged, vec!["A", "B"]);
    assert!(response.added.is_empty());
    assert!(response.removed.is_empty());
    assert!(response.changed.is_empty());
}

#[tokio::test]
async fn reload_imports_new_targets_without_removing() {
    let pages = spawn_page_fixture().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let targets_path = dir.path().join("targets.toml");
    let target_a = target("A", format!("http://{pages}/a"), "Add to cart");
    let target_b = target("B", format!("http://{pages}/b"), "Add to cart");
    let target_c = target("C", format!("http://{pages}/c"), "Add to cart");
    write_targets(&targets_path, &[target_a.clone(), target_b.clone()]);
    let (addr, _) = spawn_webwatch(targets_path.clone(), vec![target_a.clone(), target_b]).await;
    // Drop B from the file and add C; import adds C but keeps B (no purge).
    write_targets(&targets_path, &[target_a, target_c]);

    let client = reqwest::Client::new();
    let (status, body) = reload(addr).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let report: ReloadResponse = serde_json::from_str(&body).expect("json");
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let statuses = client
        .get(format!("http://{addr}/targets"))
        .bearer_auth("secret")
        .send()
        .await
        .expect("targets")
        .json::<Vec<TargetStatus>>()
        .await
        .expect("json");
    let ids = statuses
        .into_iter()
        .map(|status| status.target_id)
        .collect::<Vec<_>>();

    assert_eq!(report.added, vec!["C"]);
    assert!(report.removed.is_empty());
    assert_eq!(ids, vec!["A", "B", "C"]);
}

#[tokio::test]
async fn reload_changed_target_reports_changed() {
    let pages = spawn_page_fixture().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let targets_path = dir.path().join("targets.toml");
    let target_a = target("A", format!("http://{pages}/a"), "Add to cart");
    write_targets(&targets_path, std::slice::from_ref(&target_a));
    let (addr, _) = spawn_webwatch(targets_path.clone(), vec![target_a]).await;
    write_targets(
        &targets_path,
        &[target("A", format!("http://{pages}/a"), "Different text")],
    );

    let (status, body) = reload(addr).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let report: ReloadResponse = serde_json::from_str(&body).expect("json");

    assert_eq!(report.changed, vec!["A"]);
}

#[tokio::test]
async fn reload_parse_failure_leaves_existing_targets() {
    let pages = spawn_page_fixture().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let targets_path = dir.path().join("targets.toml");
    let target_a = target("A", format!("http://{pages}/a"), "Add to cart");
    write_targets(&targets_path, std::slice::from_ref(&target_a));
    let (addr, _) = spawn_webwatch(targets_path.clone(), vec![target_a]).await;
    std::fs::write(&targets_path, "[[targets]\nnot toml").expect("write bad targets");

    let client = reqwest::Client::new();
    let response = client
        .post(format!("http://{addr}/targets/reload"))
        .bearer_auth("secret")
        .send()
        .await
        .expect("reload");
    let statuses = client
        .get(format!("http://{addr}/targets"))
        .bearer_auth("secret")
        .send()
        .await
        .expect("targets")
        .json::<Vec<TargetStatus>>()
        .await
        .expect("json");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].target_id, "A");
}

#[tokio::test]
async fn reload_requires_bearer_auth() {
    let pages = spawn_page_fixture().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let targets_path = dir.path().join("targets.toml");
    let target_a = target("A", format!("http://{pages}/a"), "Add to cart");
    write_targets(&targets_path, std::slice::from_ref(&target_a));
    let (addr, _) = spawn_webwatch(targets_path, vec![target_a]).await;

    let response = reqwest::Client::new()
        .post(format!("http://{addr}/targets/reload"))
        .send()
        .await
        .expect("reload");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
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
        .bearer_auth("secret")
        .json(&serde_json::json!({
            "name": "Campfire Mug",
            "url": format!("http://{pages}/b"),
            "conditions": [{"kind": "text_appears", "value": "Add to cart"}],
        }))
        .send()
        .await
        .expect("create");
    assert_eq!(created.status(), StatusCode::CREATED);
    let status = created.json::<TargetStatus>().await.expect("json");
    assert_eq!(status.target_id, "campfire-mug");

    let ids = client
        .get(format!("http://{addr}/targets"))
        .bearer_auth("secret")
        .send()
        .await
        .expect("targets")
        .json::<Vec<TargetStatus>>()
        .await
        .expect("json")
        .into_iter()
        .map(|status| status.target_id)
        .collect::<Vec<_>>();
    assert!(ids.contains(&"campfire-mug".to_string()));
}

#[tokio::test]
async fn create_target_requires_auth() {
    let pages = spawn_page_fixture().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let targets_path = dir.path().join("targets.toml");
    let seed = target("A", format!("http://{pages}/a"), "Add to cart");
    write_targets(&targets_path, std::slice::from_ref(&seed));
    let (addr, _) = spawn_webwatch(targets_path, vec![seed]).await;

    let response = reqwest::Client::new()
        .post(format!("http://{addr}/targets"))
        .json(&serde_json::json!({
            "name": "No Auth",
            "url": format!("http://{pages}/b"),
            "conditions": [{"kind": "text_appears", "value": "Add to cart"}],
        }))
        .send()
        .await
        .expect("create");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
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
        .bearer_auth("secret")
        .send()
        .await
        .expect("delete");
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let ids = client
        .get(format!("http://{addr}/targets"))
        .bearer_auth("secret")
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
        .bearer_auth("secret")
        .json(&serde_json::json!({ "enabled": false }))
        .send()
        .await
        .expect("patch");
    assert_eq!(response.status(), StatusCode::OK);

    // Disabled, not deleted — still listed.
    let ids = client
        .get(format!("http://{addr}/targets"))
        .bearer_auth("secret")
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
