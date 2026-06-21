use std::{net::SocketAddr, sync::Arc};

use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::Html,
    routing::{get, post},
    Json, Router,
};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio_tungstenite::{accept_async, tungstenite::Message};
use tower::ServiceExt;
use webwatch::{
    check,
    config::EngineUsed,
    config::{
        AppConfig, BrowserConfig, Condition, ConditionRule, RenderOperation, RenderPlan,
        RenderPolicy, RenderScenario, RenderStep, RendererConfig, ScenarioMatch, SchedulerConfig,
        ServerConfig, Target,
    },
    db,
    http::HttpState,
    renderer::RendererService,
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

async fn spawn_discord_webhook() -> SocketAddr {
    let app = Router::new().route("/webhook", post(|| async { StatusCode::NO_CONTENT }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind discord fixture");
    let addr = listener.local_addr().expect("discord fixture addr");
    tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("serve discord fixture");
    });
    addr
}

async fn spawn_cdp_discovery(ws_url: String) -> SocketAddr {
    let app = Router::new().route(
        "/json/version",
        get(move || {
            let ws_url = ws_url.clone();
            async move { Json(serde_json::json!({ "webSocketDebuggerUrl": ws_url })) }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind cdp discovery fixture");
    let addr = listener.local_addr().expect("cdp discovery fixture addr");
    tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("serve cdp discovery fixture");
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
        sqlite_path: std::env::temp_dir()
            .join(format!("webwatch-test-{}", std::process::id()))
            .join("webwatch.sqlite3")
            .to_string_lossy()
            .into_owned(),
        user_agent: "webwatch-test".to_string(),
        discord_webhook_url: None,
        targets_path: Some("targets.toml".to_string()),
        server: ServerConfig::default(),
        scheduler: SchedulerConfig::default(),
        browser: BrowserConfig::default(),
        renderer: RendererConfig::default(),
    };
    let target = Target {
        id: "fixture".to_string(),
        name: "Fixture".to_string(),
        url,
        enabled: true,
        interval_secs: None,
        render: RenderPlan::default(),
        conditions,
    };
    (config, target)
}

fn enable_renderer(config: &mut AppConfig, endpoint: String) {
    config.renderer.enabled = true;
    config.renderer.endpoint = Some(endpoint);
    config.renderer.operation_timeout_ms = 1_000;
    config.renderer.navigation_timeout_ms = 1_000;
    config.renderer.settle_ms = 0;
}

async fn spawn_variant_cdp() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind variant cdp");
    let addr = listener.local_addr().expect("variant cdp addr");
    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept variant cdp");
        let mut ws = accept_async(stream).await.expect("variant cdp handshake");
        let mut selected_size = String::new();
        while let Some(message) = ws.next().await {
            let Message::Text(text) = message.expect("variant cdp message") else {
                continue;
            };
            let value: Value = serde_json::from_str(&text).expect("variant cdp json");
            let id = value.get("id").and_then(Value::as_u64).expect("id");
            let method = value.get("method").and_then(Value::as_str).expect("method");
            let response = match method {
                "Page.navigate" => {
                    selected_size.clear();
                    serde_json::json!({"id": id, "result": {}})
                }
                "Runtime.evaluate" => {
                    let expression = value
                        .get("params")
                        .and_then(|params| params.get("expression"))
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let result = if expression == "document.documentElement.outerHTML" {
                        if selected_size == "Huge" {
                            "<html><body><button class='buy'>Available</button></body></html>"
                        } else {
                            "<html><body><button disabled>Sold out</button></body></html>"
                        }
                    } else if expression == "location.href" {
                        "https://rendered.example/final"
                    } else {
                        if expression.contains("\"option_text\":\"Huge\"") {
                            selected_size = "Huge".to_string();
                            "selected Huge"
                        } else if expression.contains("\"option_text\":\"Medium\"") {
                            selected_size = "Medium".to_string();
                            "selected Medium"
                        } else {
                            "step ok"
                        }
                    };
                    serde_json::json!({"id": id, "result": {"result": {"value": result}}})
                }
                _ => serde_json::json!({"id": id, "result": {}}),
            };
            ws.send(Message::Text(response.to_string().into()))
                .await
                .expect("variant cdp response");
        }
    });
    format!("ws://{addr}")
}

async fn spawn_fake_cdp(html: &'static str) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind fake cdp");
    let addr = listener.local_addr().expect("fake cdp addr");
    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept fake cdp");
        let mut ws = accept_async(stream).await.expect("fake cdp handshake");
        while let Some(message) = ws.next().await {
            let Message::Text(text) = message.expect("fake cdp message") else {
                continue;
            };
            let value: Value = serde_json::from_str(&text).expect("fake cdp json");
            let id = value.get("id").and_then(Value::as_u64).expect("id");
            let method = value.get("method").and_then(Value::as_str).expect("method");
            let response = match method {
                "Browser.getVersion" => serde_json::json!({
                    "id": id,
                    "result": {"product": "Chrome/Test", "protocolVersion": "1.3"}
                }),
                "Runtime.evaluate" => {
                    let expression = value
                        .get("params")
                        .and_then(|params| params.get("expression"))
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let result = if expression == "location.href" {
                        "https://rendered.example/final"
                    } else {
                        html
                    };
                    serde_json::json!({"id": id, "result": {"result": {"value": result}}})
                }
                _ => serde_json::json!({"id": id, "result": {}}),
            };
            ws.send(Message::Text(response.to_string().into()))
                .await
                .expect("fake cdp response");
        }
    });
    format!("ws://{addr}")
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
    let renderer = RendererService::from_config(&config);

    let outcome = check::check_target(&config, &client, &renderer, target)
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
    let renderer = RendererService::from_config(&config);

    let outcome = check::check_target(&config, &client, &renderer, target)
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
    let renderer = RendererService::from_config(&config);

    let error = check::check_target(&config, &client, &renderer, target)
        .await
        .expect_err("HTTP should need browser rendering");

    assert!(error.to_string().contains("browser rendering required"));
}

#[tokio::test]
async fn http_only_policy_never_uses_renderer() {
    let addr = spawn_fixture().await;
    let (mut config, mut target_config) = config_for(
        format!("http://{addr}/js-rendered"),
        vec![Condition {
            id: Some("button".to_string()),
            rule: ConditionRule::Selector {
                selector: "button.buy".to_string(),
                negate: false,
            },
        }],
    );
    enable_renderer(&mut config, "ws://127.0.0.1:1".to_string());
    target_config.render.policy = RenderPolicy::HttpOnly;
    let config = config.resolve_env().expect("valid config");
    let target = target_config.validated().expect("valid target");
    let client = reqwest::Client::new();
    let renderer = RendererService::from_config(&config);

    let error = check::check_target(&config, &client, &renderer, target)
        .await
        .expect_err("HTTP-only should not render");

    assert!(error.to_string().contains("browser rendering required"));
}

#[tokio::test]
async fn auto_policy_falls_back_to_renderer_for_js_shell() {
    let addr = spawn_fixture().await;
    let cdp_url =
        spawn_fake_cdp("<html><body><button class='buy'>Add to cart</button></body></html>").await;
    let (mut config, target_config) = config_for(
        format!("http://{addr}/js-rendered"),
        vec![Condition {
            id: Some("button".to_string()),
            rule: ConditionRule::Selector {
                selector: "button.buy".to_string(),
                negate: false,
            },
        }],
    );
    enable_renderer(&mut config, cdp_url);
    let config = config.resolve_env().expect("valid config");
    let target = target_config.validated().expect("valid target");
    let client = reqwest::Client::new();
    let renderer = RendererService::from_config(&config);

    let outcome = check::check_target(&config, &client, &renderer, target)
        .await
        .expect("auto fallback");

    assert!(outcome.matched);
    assert_eq!(outcome.engine_used, EngineUsed::BrowserCdp);
}

#[tokio::test]
async fn auto_policy_falls_back_to_renderer_when_http_is_blocked() {
    // A retail site that refuses the plain HTTP request (bot blocking) produces a
    // transport error, not a JS shell. Auto must still escalate to the renderer.
    let cdp_url =
        spawn_fake_cdp("<html><body><button class='buy'>Add to cart</button></body></html>").await;
    let (mut config, target_config) = config_for(
        "http://127.0.0.1:1/blocked".to_string(),
        vec![Condition {
            id: Some("button".to_string()),
            rule: ConditionRule::Selector {
                selector: "button.buy".to_string(),
                negate: false,
            },
        }],
    );
    enable_renderer(&mut config, cdp_url);
    // policy stays at the default (auto)
    let config = config.resolve_env().expect("valid config");
    let target = target_config.validated().expect("valid target");
    let client = reqwest::Client::new();
    let renderer = RendererService::from_config(&config);

    let outcome = check::check_target(&config, &client, &renderer, target)
        .await
        .expect("auto should fall back to renderer when HTTP is blocked");

    assert!(outcome.matched);
    assert_eq!(outcome.engine_used, EngineUsed::BrowserCdp);
}

async fn spawn_nav_error_cdp(error_text: &'static str) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind nav error cdp");
    let addr = listener.local_addr().expect("nav error cdp addr");
    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept nav error cdp");
        let mut ws = accept_async(stream).await.expect("nav error cdp handshake");
        while let Some(message) = ws.next().await {
            let Message::Text(text) = message.expect("nav error cdp message") else {
                continue;
            };
            let value: Value = serde_json::from_str(&text).expect("nav error cdp json");
            let id = value.get("id").and_then(Value::as_u64).expect("id");
            let method = value.get("method").and_then(Value::as_str).expect("method");
            let response = match method {
                "Page.navigate" => {
                    serde_json::json!({"id": id, "result": {"errorText": error_text}})
                }
                "Runtime.evaluate" => {
                    let expression = value
                        .get("params")
                        .and_then(|params| params.get("expression"))
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let result = if expression == "location.href" {
                        "chrome-error://chromewebdata/"
                    } else {
                        "<html><body>This site can’t be reached</body></html>"
                    };
                    serde_json::json!({"id": id, "result": {"result": {"value": result}}})
                }
                _ => serde_json::json!({"id": id, "result": {}}),
            };
            ws.send(Message::Text(response.to_string().into()))
                .await
                .expect("nav error cdp response");
        }
    });
    format!("ws://{addr}")
}

#[tokio::test]
async fn render_navigation_failure_is_reported_as_error() {
    // Best Buy-style hard failure: Page.navigate reports errorText and the page
    // lands on chrome-error://. The check must surface an error, never a match —
    // a `text_disappears` condition would otherwise read the error page as a hit.
    let cdp_url = spawn_nav_error_cdp("net::ERR_HTTP2_PROTOCOL_ERROR").await;
    let (mut config, mut target_config) = config_for(
        "https://blocked.example/product".to_string(),
        vec![Condition {
            id: Some("not-unavailable".to_string()),
            rule: ConditionRule::Text {
                value: "Unavailable".to_string(),
                negate: true,
            },
        }],
    );
    enable_renderer(&mut config, cdp_url);
    target_config.render.policy = RenderPolicy::RenderFirst;
    let config = config.resolve_env().expect("valid config");
    let target = target_config.validated().expect("valid target");
    let client = reqwest::Client::new();
    let renderer = RendererService::from_config(&config);

    let error = check::check_target(&config, &client, &renderer, target)
        .await
        .expect_err("navigation failure must not produce a match");

    let message = error.to_string();
    assert!(
        message.contains("navigation") && message.contains("ERR_HTTP2_PROTOCOL_ERROR"),
        "unexpected error: {message}"
    );
}

#[tokio::test]
async fn render_first_policy_skips_http() {
    let cdp_url =
        spawn_fake_cdp("<html><body><button class='buy'>Add to cart</button></body></html>").await;
    let (mut config, mut target_config) = config_for(
        "http://127.0.0.1:1/unreachable".to_string(),
        vec![Condition {
            id: Some("button".to_string()),
            rule: ConditionRule::Selector {
                selector: "button.buy".to_string(),
                negate: false,
            },
        }],
    );
    enable_renderer(&mut config, cdp_url);
    target_config.render.policy = RenderPolicy::RenderFirst;
    let config = config.resolve_env().expect("valid config");
    let target = target_config.validated().expect("valid target");
    let client = reqwest::Client::new();
    let renderer = RendererService::from_config(&config);

    let outcome = check::check_target(&config, &client, &renderer, target)
        .await
        .expect("render first");

    assert!(outcome.matched);
    assert_eq!(outcome.engine_used, EngineUsed::BrowserCdp);
}

#[tokio::test]
async fn render_scenarios_evaluate_each_variant() {
    let cdp_url = spawn_variant_cdp().await;
    let (mut config, mut target_config) = config_for(
        "http://127.0.0.1:1/unreachable".to_string(),
        vec![Condition {
            id: Some("available".to_string()),
            rule: ConditionRule::Selector {
                selector: "button.buy".to_string(),
                negate: false,
            },
        }],
    );
    enable_renderer(&mut config, cdp_url);
    target_config.render.policy = RenderPolicy::RenderFirst;
    target_config.render.scenario_match = ScenarioMatch::Any;
    target_config.render.steps = vec![RenderStep {
        op: RenderOperation::WaitForText,
        selector: None,
        text: Some("Bartholomew Bear".to_string()),
        option_text: None,
        option_value: None,
        value: None,
        timeout_ms: Some(100),
        settle_ms: None,
    }];
    target_config.render.scenarios = vec![
        RenderScenario {
            id: "medium".to_string(),
            label: "Medium".to_string(),
            steps: vec![RenderStep {
                op: RenderOperation::Select,
                selector: Some("select[name*='Size']".to_string()),
                text: None,
                option_text: Some("Medium".to_string()),
                option_value: None,
                value: None,
                timeout_ms: Some(100),
                settle_ms: None,
            }],
        },
        RenderScenario {
            id: "huge".to_string(),
            label: "Huge".to_string(),
            steps: vec![RenderStep {
                op: RenderOperation::Select,
                selector: Some("select[name*='Size']".to_string()),
                text: None,
                option_text: Some("Huge".to_string()),
                option_value: None,
                value: None,
                timeout_ms: Some(100),
                settle_ms: None,
            }],
        },
    ];
    let config = config.resolve_env().expect("valid config");
    let target = target_config.validated().expect("valid target");
    let client = reqwest::Client::new();
    let renderer = RendererService::from_config(&config);

    let outcome = check::check_target(&config, &client, &renderer, target)
        .await
        .expect("scenario render");

    assert!(outcome.matched);
    assert_eq!(outcome.condition_results.len(), 2);
    assert!(outcome.condition_results.iter().any(|result| {
        result.matched
            && result.scenario_id.as_deref() == Some("huge")
            && result.scenario_label.as_deref() == Some("Huge")
    }));
    assert!(outcome.evidence.iter().any(|entry| entry.contains("Huge:")));
}

// ---------------------------------------------------------------------------
// HTTP API + SPA fallback tests
// ---------------------------------------------------------------------------

fn router_config() -> AppConfig {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir
        .keep()
        .join(format!("test-{}.sqlite3", db::backend_name()));
    AppConfig {
        sqlite_path: db_path.to_string_lossy().to_string(),
        user_agent: "webwatch-test".to_string(),
        discord_webhook_url: None,
        targets_path: None,
        server: ServerConfig::default(),
        scheduler: SchedulerConfig::default(),
        browser: BrowserConfig::default(),
        renderer: RendererConfig::default(),
    }
}

async fn build_router() -> Router {
    build_router_with_config(router_config()).await
}

async fn build_router_with_config(config: AppConfig) -> Router {
    let config = Arc::new(config);
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
async fn health_reports_renderer_state() {
    let app = build_router().await;
    let res = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), axum::http::StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).expect("health json");
    assert_eq!(json["status"], "ok");
    assert_eq!(json["renderer_enabled"], false);
    assert_eq!(json["renderer_configured"], false);
    assert_eq!(json["renderer_backend"], "cloakbrowser");
}

#[tokio::test]
async fn discord_test_endpoint_reports_success_without_leaking_webhook() {
    let discord_addr = spawn_discord_webhook().await;
    let mut config = router_config();
    config.discord_webhook_url = Some(format!("http://{discord_addr}/webhook"));
    let app = build_router_with_config(config).await;

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/ops/discord/test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).expect("discord test json");

    assert_eq!(json["configured"], true);
    assert_eq!(json["ok"], true);
    assert_eq!(json["checks"][0]["name"], "webhook_config");
    assert!(!String::from_utf8_lossy(&body).contains(&discord_addr.to_string()));
}

#[tokio::test]
async fn discord_test_endpoint_reports_missing_webhook_as_diagnostic() {
    let app = build_router().await;

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/ops/discord/test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).expect("discord test json");

    assert_eq!(json["configured"], false);
    assert_eq!(json["ok"], false);
    assert_eq!(json["checks"][0]["name"], "webhook_config");
}

#[tokio::test]
async fn discord_test_endpoint_redacts_failed_webhook_url() {
    let mut config = router_config();
    config.discord_webhook_url = Some("http://127.0.0.1:1/webhook?token=secret".to_string());
    let app = build_router_with_config(config).await;

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/ops/discord/test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).expect("discord test json");

    assert_eq!(json["configured"], true);
    assert_eq!(json["ok"], false);
    let body = String::from_utf8_lossy(&body);
    assert!(!body.contains("127.0.0.1"), "{body}");
    assert!(!body.contains("secret"), "{body}");
}

#[tokio::test]
async fn renderer_test_endpoint_reports_cdp_connection_success() {
    let ws_url = spawn_fake_cdp("<html><body>diagnostic</body></html>").await;
    let discovery_addr = spawn_cdp_discovery(ws_url).await;
    let mut config = router_config();
    config.renderer.enabled = true;
    config.renderer.endpoint = Some(format!("http://{discovery_addr}"));
    let app = build_router_with_config(config).await;

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/ops/renderer/test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).expect("renderer test json");

    assert_eq!(json["configured"], true);
    assert_eq!(json["ok"], true);
    let checks = json["checks"].as_array().expect("checks");
    assert!(checks
        .iter()
        .any(|check| check["name"] == "discovery" && check["ok"] == true));
    assert!(checks
        .iter()
        .any(|check| check["name"] == "browser_version" && check["ok"] == true));
}

#[tokio::test]
async fn serves_spa_index_for_unknown_get() {
    let app = build_router().await;
    let res = app
        .oneshot(
            Request::builder()
                .uri("/watches/some-id")
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
async fn operations_ui_route_is_not_shadowed_by_ops_api() {
    let app = build_router().await;
    let res = app
        .oneshot(
            Request::builder()
                .uri("/operations")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let ct = res.headers().get(axum::http::header::CONTENT_TYPE).cloned();
    assert!(
        ct.map(|v| v.to_str().unwrap_or("").contains("text/html"))
            .unwrap_or(false),
        "expected text/html content-type from /operations UI route"
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
