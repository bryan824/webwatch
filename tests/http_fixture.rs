use std::{net::SocketAddr, sync::Arc};

use axum::{body::Body, http::Request, response::Html, routing::get, Router};
use tower::ServiceExt;
use webwatch::{
    config::EngineUsed,
    config::{
        AppConfig, BrowserConfig, Condition, ConditionRule, SchedulerConfig, ServerConfig, Target,
    },
    db, evaluator,
    http::HttpState,
    scheduler::Scheduler,
};

async fn spawn_fixture() -> SocketAddr {
    let app = Router::new()
        .route("/static-in-stock", get(static_in_stock))
        .route("/static-sold-out", get(static_sold_out))
        .route("/js-rendered", get(js_rendered));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind fixture server");
    let addr = listener.local_addr().expect("fixture addr");
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve fixture");
    });
    addr
}

async fn static_in_stock() -> Html<&'static str> {
    Html(
        r#"
        <!doctype html>
        <html>
            <body>
                <h1>Campfire Mug</h1>
                <p class="price">$42.50</p>
                <button class="buy">Add to cart</button>
            </body>
        </html>
        "#,
    )
}

async fn static_sold_out() -> Html<&'static str> {
    Html(
        r#"
        <!doctype html>
        <html>
            <body>
                <h1>Campfire Mug</h1>
                <p class="price">$42.50</p>
                <button class="buy" disabled>Sold out</button>
            </body>
        </html>
        "#,
    )
}

async fn js_rendered() -> Html<&'static str> {
    Html(
        r#"
        <!doctype html>
        <html>
            <body>
                <div id="app"></div>
                <script>
                    setTimeout(() => {
                        document.querySelector('#app').innerHTML = '<button class="buy">Add to cart</button>';
                    }, 100);
                </script>
            </body>
        </html>
        "#,
    )
}

fn config_for(url: String, conditions: Vec<Condition>) -> (AppConfig, Target) {
    let config = AppConfig {
        sqlite_path: "webwatch.sqlite3".to_string(),
        user_agent: "webwatch-test".to_string(),
        discord_webhook_url: None,
        targets_path: Some("targets.toml".to_string()),
        server: ServerConfig::default(),
        scheduler: SchedulerConfig::default(),
        browser: BrowserConfig::default(),
    };
    let target = Target {
        id: "fixture".to_string(),
        name: "Fixture".to_string(),
        url,
        enabled: true,
        interval_secs: None,
        conditions,
    };
    (config, target)
}

#[tokio::test]
async fn http_engine_matches_text_selector_and_price_conditions() {
    let addr = spawn_fixture().await;
    let (config, target_config) = config_for(
        format!("http://{addr}/static-in-stock"),
        vec![
            Condition {
                id: Some("text".to_string()),
                rule: ConditionRule::Text {
                    value: "Add to cart".to_string(),
                    negate: false,
                },
            },
            Condition {
                id: Some("button".to_string()),
                rule: ConditionRule::Selector {
                    selector: "button.buy".to_string(),
                    negate: false,
                },
            },
            Condition {
                id: Some("price".to_string()),
                rule: ConditionRule::Price {
                    threshold_cents: 5_000,
                    selector: None,
                    price_selector: Some(".price".to_string()),
                    negate: false,
                },
            },
        ],
    );
    let config = config.resolve_env().expect("valid config");
    let target = target_config.validated().expect("valid target");
    let client = reqwest::Client::new();

    let outcome = evaluator::check_target(&config, &client, target)
        .await
        .expect("check target");

    assert!(outcome.matched);
    assert_eq!(outcome.engine_used, EngineUsed::Http);
    assert_eq!(outcome.price_cents, Some(4_250));
    assert_eq!(outcome.condition_results.len(), 3);
}

#[tokio::test]
async fn http_engine_detects_sold_out_text_disappeared_condition() {
    let addr = spawn_fixture().await;
    let (config, target_config) = config_for(
        format!("http://{addr}/static-sold-out"),
        vec![Condition {
            id: Some("not-available".to_string()),
            rule: ConditionRule::Text {
                value: "Add to cart".to_string(),
                negate: true,
            },
        }],
    );
    let config = config.resolve_env().expect("valid config");
    let target = target_config.validated().expect("valid target");
    let client = reqwest::Client::new();

    let outcome = evaluator::check_target(&config, &client, target)
        .await
        .expect("check target");

    assert!(outcome.matched);
    assert_eq!(outcome.engine_used, EngineUsed::Http);
}

#[tokio::test]
async fn js_rendered_page_requests_browser_when_http_cannot_prove_condition() {
    let addr = spawn_fixture().await;
    let (config, target_config) = config_for(
        format!("http://{addr}/js-rendered"),
        vec![Condition {
            id: Some("button".to_string()),
            rule: ConditionRule::Selector {
                selector: "button.buy".to_string(),
                negate: false,
            },
        }],
    );
    let config = config.resolve_env().expect("valid config");
    let target = target_config.validated().expect("valid target");
    let client = reqwest::Client::new();

    let error = evaluator::check_target(&config, &client, target)
        .await
        .expect_err("HTTP should need browser rendering");

    assert!(error.to_string().contains("browser rendering required"));
}

// ---------------------------------------------------------------------------
// HTTP API + SPA fallback tests
// ---------------------------------------------------------------------------

async fn build_router() -> Router {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir
        .keep()
        .join(format!("test-{}.sqlite3", db::backend_name()));
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
    let client = reqwest::Client::new();
    let scheduler = Arc::new(Scheduler::new(
        config.clone(),
        persistence.clone(),
        client.clone(),
    ));
    scheduler.start(&[]).await;
    let state = HttpState {
        config,
        scheduler,
        db: persistence,
        client,
    };
    webwatch::http::router(state)
}

#[tokio::test]
async fn serves_spa_index_for_unknown_get() {
    let app = build_router().await;
    let res = app
        .oneshot(
            Request::builder()
                .uri("/targets/some-id")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), axum::http::StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let text = String::from_utf8_lossy(&body);
    assert!(
        text.contains("<!DOCTYPE html>")
            || text.contains("<!doctype html>")
            || text.starts_with("<!"),
        "expected HTML document, got: {}",
        &text[..text.len().min(200)]
    );
}

#[tokio::test]
async fn api_routes_are_not_shadowed_by_spa() {
    let app = build_router().await;
    let res = app
        .oneshot(
            Request::builder()
                .uri("/targets")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    // /targets is a JSON API endpoint — must NOT be served as SPA HTML
    let ct = res.headers().get(axum::http::header::CONTENT_TYPE).cloned();
    assert!(
        ct.map(|v| v.to_str().unwrap_or("").contains("application/json"))
            .unwrap_or(false),
        "expected application/json content-type from /targets API route"
    );
}
