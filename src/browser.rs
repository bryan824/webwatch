use crate::{
    config::AppConfig,
    config::{CheckOutcome, EngineUsed, Target},
    evaluator,
    renderer::{RenderRequest, RendererService},
};

pub async fn check_with_browser(config: &AppConfig, target: Target) -> crate::Result<CheckOutcome> {
    let renderer = RendererService::from_config(config);
    let snapshot = renderer
        .render(RenderRequest {
            target_id: target.id.clone(),
            url: target.url.clone(),
            plan: target.render.clone(),
        })
        .await?;
    evaluator::evaluate_document(
        target,
        EngineUsed::BrowserCdp,
        &snapshot.html,
        &snapshot.final_url,
    )
}

#[cfg(test)]
mod tests {
    use crate::config::{AppConfig, BrowserConfig, RendererConfig, SchedulerConfig, ServerConfig};

    #[test]
    fn browser_config_can_parse_cdp_url() {
        let raw = r#"
            [browser]
            cdp_url = "ws://127.0.0.1:9222"
            wait_ms = 250

            [[targets]]
            id = "target"
            name = "Target"
            url = "https://example.com"

            [[targets.conditions]]
            kind = "text_appears"
            value = "Add to cart"
        "#;
        let config: AppConfig = toml::from_str(raw).expect("parse config");

        assert_eq!(
            config.browser.cdp_url.as_deref(),
            Some("ws://127.0.0.1:9222")
        );
        assert_eq!(config.browser.wait_ms, 250);
    }

    #[test]
    fn app_config_defaults_browser_to_optional() {
        let config = AppConfig {
            sqlite_path: "webwatch.sqlite3".to_string(),
            user_agent: "test".to_string(),
            discord_webhook_url: None,
            targets_path: Some("targets.toml".to_string()),
            server: ServerConfig::default(),
            scheduler: SchedulerConfig::default(),
            browser: BrowserConfig::default(),
            renderer: RendererConfig::default(),
        };

        assert!(config.browser.cdp_url.is_none());
    }
}
