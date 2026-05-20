use std::net::SocketAddr;

use axum::{response::Html, routing::get, Router};
use webwatch::{
    config::{
        AppConfig, BrowserConfig, ConditionConfig, SchedulerConfig, ServerConfig, TargetConfig,
    },
    config::{ConditionKind, EngineUsed},
    evaluator,
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

fn config_for(url: String, conditions: Vec<ConditionConfig>) -> AppConfig {
    AppConfig {
        sqlite_path: "webwatch.sqlite3".to_string(),
        user_agent: "webwatch-test".to_string(),
        discord_webhook_url: None,
        api_token: None,
        server: ServerConfig::default(),
        scheduler: SchedulerConfig::default(),
        browser: BrowserConfig::default(),
        targets: vec![TargetConfig {
            id: "fixture".to_string(),
            name: "Fixture".to_string(),
            url,
            enabled: true,
            interval_secs: None,
            conditions,
        }],
    }
}

#[tokio::test]
async fn http_engine_matches_text_selector_and_price_conditions() {
    let addr = spawn_fixture().await;
    let config = config_for(
        format!("http://{addr}/static-in-stock"),
        vec![
            ConditionConfig {
                id: Some("text".to_string()),
                kind: ConditionKind::TextAppears,
                value: Some("Add to cart".to_string()),
                selector: None,
                threshold_cents: None,
                price_selector: None,
            },
            ConditionConfig {
                id: Some("button".to_string()),
                kind: ConditionKind::SelectorExists,
                value: None,
                selector: Some("button.buy".to_string()),
                threshold_cents: None,
                price_selector: None,
            },
            ConditionConfig {
                id: Some("price".to_string()),
                kind: ConditionKind::PriceBelow,
                value: None,
                selector: None,
                threshold_cents: Some(5_000),
                price_selector: Some(".price".to_string()),
            },
        ],
    )
    .resolve_env_and_validate()
    .expect("valid config");
    let target = config.targets[0].to_target().expect("target");
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
    let config = config_for(
        format!("http://{addr}/static-sold-out"),
        vec![ConditionConfig {
            id: Some("not-available".to_string()),
            kind: ConditionKind::TextDisappears,
            value: Some("Add to cart".to_string()),
            selector: None,
            threshold_cents: None,
            price_selector: None,
        }],
    )
    .resolve_env_and_validate()
    .expect("valid config");
    let target = config.targets[0].to_target().expect("target");
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
    let config = config_for(
        format!("http://{addr}/js-rendered"),
        vec![ConditionConfig {
            id: Some("button".to_string()),
            kind: ConditionKind::SelectorExists,
            value: None,
            selector: Some("button.buy".to_string()),
            threshold_cents: None,
            price_selector: None,
        }],
    )
    .resolve_env_and_validate()
    .expect("valid config");
    let target = config.targets[0].to_target().expect("target");
    let client = reqwest::Client::new();

    let error = evaluator::check_target(&config, &client, target)
        .await
        .expect_err("HTTP should need browser rendering");

    assert!(error.to_string().contains("browser rendering required"));
}
