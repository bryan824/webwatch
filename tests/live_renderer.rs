//! Ignored live checks for the optional CloakBrowser/CDP renderer.
//!
//! Run with a local CloakBrowser `cloakserve` endpoint, for example:
//!
//! ```bash
//! docker run -d --name webwatch-cloak -p 127.0.0.1:9222:9222 \
//!   cloakhq/cloakbrowser cloakserve --idle-timeout=300
//! WEBWATCH_LIVE_CDP_ENDPOINT=http://127.0.0.1:9222 \
//!   cargo test --test live_renderer -- --ignored --nocapture --test-threads=1
//! ```

use webwatch::{
    check,
    config::{
        AppConfig, BrowserConfig, Condition, ConditionRule, EngineUsed, RenderOperation,
        RenderPlan, RenderPolicy, RenderScenario, RenderStep, RendererBackend, RendererConfig,
        ScenarioMatch, SchedulerConfig, ServerConfig, Target,
    },
    renderer::RendererService,
};

fn live_endpoint() -> Option<String> {
    std::env::var("WEBWATCH_LIVE_CDP_ENDPOINT")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| std::env::var("CLOAKBROWSER_ENDPOINT").ok())
        .filter(|value| !value.trim().is_empty())
}

fn live_config(endpoint: String) -> AppConfig {
    AppConfig {
        sqlite_path: "webwatch-live.sqlite3".to_string(),
        user_agent: "webwatch-live-check/0.1".to_string(),
        discord_webhook_url: None,
        targets_path: None,
        server: ServerConfig::default(),
        scheduler: SchedulerConfig::default(),
        browser: BrowserConfig::default(),
        renderer: RendererConfig {
            enabled: true,
            backend: RendererBackend::CloakBrowser,
            endpoint: Some(endpoint),
            max_concurrency: 1,
            navigation_timeout_ms: 15_000,
            operation_timeout_ms: 30_000,
            settle_ms: 1_000,
        },
    }
}

fn step(op: RenderOperation) -> RenderStep {
    RenderStep {
        op,
        selector: None,
        text: None,
        option_text: None,
        option_value: None,
        value: None,
        timeout_ms: None,
        settle_ms: None,
    }
}

async fn run_live_check(
    endpoint: String,
    target: Target,
) -> Result<webwatch::config::CheckOutcome, Box<dyn std::error::Error>> {
    let config = live_config(endpoint).resolve_env()?;
    let client = reqwest::Client::builder()
        .user_agent(config.user_agent.clone())
        .timeout(config.http_timeout())
        .build()?;
    let renderer = RendererService::from_config(&config);
    Ok(check::check_target(&config, &client, &renderer, target.validated()?).await?)
}

#[tokio::test]
#[ignore = "requires a live CloakBrowser/CDP endpoint and internet access"]
async fn bestbuy_open_box_page_renders_with_cloakbrowser() -> Result<(), Box<dyn std::error::Error>>
{
    let Some(endpoint) = live_endpoint() else {
        eprintln!("skipping: set WEBWATCH_LIVE_CDP_ENDPOINT=http://127.0.0.1:9222");
        return Ok(());
    };
    let seed = format!("bestbuy-airtag-open-box-live-{}", std::process::id());
    let mut render = RenderPlan {
        policy: RenderPolicy::RenderFirst,
        fingerprint_seed: Some(seed.clone()),
        wait_ms: Some(5_000),
        ..RenderPlan::default()
    };
    let mut wait = step(RenderOperation::WaitForText);
    wait.text = Some("AirTag".to_string());
    wait.timeout_ms = Some(30_000);
    render.steps.push(wait);

    let outcome = run_live_check(
        endpoint,
        Target {
            id: seed,
            name: "Best Buy AirTag Open-Box Live".to_string(),
            url: "https://www.bestbuy.com/product/apple-airtag-4-pack-1st-generation-2021-silver/JJGCQ8XFQH/sku/6461349/openbox?condition=good".to_string(),
            enabled: true,
            interval_secs: None,
            render,
            conditions: vec![Condition {
                id: Some("product-title".to_string()),
                rule: ConditionRule::Text {
                    value: "AirTag".to_string(),
                    negate: false,
                },
            }],
        },
    )
    .await?;

    eprintln!("{}", serde_json::to_string_pretty(&outcome)?);
    assert_eq!(outcome.engine_used, EngineUsed::BrowserCdp);
    assert!(
        outcome.matched,
        "Best Buy open-box URL evidence: {:?}",
        outcome.evidence
    );
    Ok(())
}

#[tokio::test]
#[ignore = "requires a live CloakBrowser/CDP endpoint and internet access"]
async fn jellycat_variants_are_checked_per_scenario() -> Result<(), Box<dyn std::error::Error>> {
    let Some(endpoint) = live_endpoint() else {
        eprintln!("skipping: set WEBWATCH_LIVE_CDP_ENDPOINT=http://127.0.0.1:9222");
        return Ok(());
    };
    let seed = format!("jellycat-bartholomew-live-{}", std::process::id());
    let mut render = RenderPlan {
        policy: RenderPolicy::RenderFirst,
        fingerprint_seed: Some(seed.clone()),
        wait_ms: Some(1_000),
        scenario_match: ScenarioMatch::Any,
        ..RenderPlan::default()
    };
    let mut wait = step(RenderOperation::WaitForText);
    wait.text = Some("Bartholomew Bear".to_string());
    wait.timeout_ms = Some(30_000);
    render.steps.push(wait);
    let mut wait_for_options = step(RenderOperation::WaitFor);
    wait_for_options.selector = Some("select#attribute_select_152 option[value='144']".to_string());
    wait_for_options.timeout_ms = Some(30_000);
    render.steps.push(wait_for_options);

    let mut tiny_select = step(RenderOperation::Select);
    tiny_select.selector = Some("select#attribute_select_152".to_string());
    tiny_select.option_value = Some("344".to_string());
    tiny_select.timeout_ms = Some(20_000);
    tiny_select.settle_ms = Some(1_000);

    let mut huge_select = step(RenderOperation::Select);
    huge_select.selector = Some("select#attribute_select_152".to_string());
    huge_select.option_value = Some("144".to_string());
    huge_select.timeout_ms = Some(20_000);
    huge_select.settle_ms = Some(3_000);
    let mut huge_wait_available = step(RenderOperation::WaitFor);
    huge_wait_available.selector = Some("#form-action-addToCart[value=\"Add to Bag\"]".to_string());
    huge_wait_available.timeout_ms = Some(20_000);

    render.scenarios = vec![
        RenderScenario {
            id: "tiny".to_string(),
            label: "Tiny".to_string(),
            steps: vec![tiny_select],
        },
        RenderScenario {
            id: "huge".to_string(),
            label: "Huge".to_string(),
            steps: vec![huge_select, huge_wait_available],
        },
    ];

    let outcome = run_live_check(
        endpoint,
        Target {
            id: seed,
            name: "Jellycat Bartholomew Bear Live".to_string(),
            url: "https://us.jellycat.com/bartholomew-bear/".to_string(),
            enabled: true,
            interval_secs: None,
            render,
            conditions: vec![Condition {
                id: Some("available".to_string()),
                rule: ConditionRule::Selector {
                    selector: "#form-action-addToCart[value=\"Add to Bag\"]".to_string(),
                    negate: false,
                },
            }],
        },
    )
    .await?;

    eprintln!("{}", serde_json::to_string_pretty(&outcome)?);
    assert_eq!(outcome.engine_used, EngineUsed::BrowserCdp);
    assert!(outcome.matched, "Jellycat evidence: {:?}", outcome.evidence);
    assert!(outcome
        .condition_results
        .iter()
        .any(|result| { result.scenario_id.as_deref() == Some("huge") && result.matched }));
    assert!(outcome
        .condition_results
        .iter()
        .any(|result| { result.scenario_id.as_deref() == Some("tiny") && !result.matched }));
    Ok(())
}
