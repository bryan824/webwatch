use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use snafu::OptionExt;
use tokio_tungstenite::{connect_async, tungstenite::Message, WebSocketStream};

use crate::{
    config::AppConfig,
    config::{CheckOutcome, EngineUsed, Target},
    error::{BrowserProtocolSnafu, BrowserResponseMissingSnafu, MissingBrowserCdpUrlSnafu, Result},
    evaluator,
};

pub async fn check_with_browser(config: &AppConfig, target: Target) -> Result<CheckOutcome> {
    let html = fetch_rendered_html(config, &target.url).await?;
    evaluator::evaluate_document(target, EngineUsed::BrowserCdp, &html)
}

async fn fetch_rendered_html(config: &AppConfig, url: &str) -> Result<String> {
    let cdp_url = config
        .browser
        .cdp_url
        .as_deref()
        .context(MissingBrowserCdpUrlSnafu)?;
    let (stream, _) =
        connect_async(cdp_url)
            .await
            .map_err(|error| crate::Error::BrowserConnect {
                url: cdp_url.to_string(),
                message: error.to_string(),
            })?;
    let mut client = CdpClient { stream, next_id: 1 };

    client.command("Page.enable", json!({})).await?;
    client.command("Runtime.enable", json!({})).await?;
    client
        .command("Page.navigate", json!({ "url": url }))
        .await?;
    tokio::time::sleep(Duration::from_millis(config.browser.wait_ms)).await;
    let result = client
        .command(
            "Runtime.evaluate",
            json!({
                "expression": "document.documentElement.outerHTML",
                "returnByValue": true,
            }),
        )
        .await?;

    result
        .get("result")
        .and_then(|value| value.get("result"))
        .and_then(|value| value.get("value"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| crate::Error::BrowserResponseMissing {
            method: "Runtime.evaluate".to_string(),
            field: "result.result.value".to_string(),
        })
}

type MaybeTlsStream = tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>;

struct CdpClient {
    stream: WebSocketStream<MaybeTlsStream>,
    next_id: u64,
}

impl CdpClient {
    async fn command(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        let message = json!({
            "id": id,
            "method": method,
            "params": params,
        });
        self.stream
            .send(Message::Text(message.to_string().into()))
            .await
            .map_err(|error| crate::Error::BrowserSend {
                method: method.to_string(),
                message: error.to_string(),
            })?;

        while let Some(message) = self.stream.next().await {
            let message = message.map_err(|error| crate::Error::BrowserRead {
                method: method.to_string(),
                message: error.to_string(),
            })?;
            let Message::Text(text) = message else {
                continue;
            };
            let value: Value =
                serde_json::from_str(&text).map_err(|error| crate::Error::BrowserProtocol {
                    method: method.to_string(),
                    message: format!("invalid JSON response: {error}"),
                })?;
            if value.get("id").and_then(Value::as_u64) != Some(id) {
                continue;
            }
            if let Some(error) = value.get("error") {
                return BrowserProtocolSnafu {
                    method: method.to_string(),
                    message: error.to_string(),
                }
                .fail();
            }
            return Ok(value);
        }

        BrowserResponseMissingSnafu {
            method: method.to_string(),
            field: "response".to_string(),
        }
        .fail()
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        config::ConditionKind,
        config::{
            AppConfig, BrowserConfig, ConditionConfig, SchedulerConfig, ServerConfig, TargetConfig,
        },
    };

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
            api_token: None,
            server: ServerConfig::default(),
            scheduler: SchedulerConfig::default(),
            browser: BrowserConfig::default(),
            targets: vec![TargetConfig {
                id: "target".to_string(),
                name: "Target".to_string(),
                url: "https://example.com".to_string(),
                enabled: true,
                interval_secs: None,
                conditions: vec![ConditionConfig {
                    id: None,
                    kind: ConditionKind::TextAppears,
                    value: Some("Add to cart".to_string()),
                    selector: None,
                    threshold_cents: None,
                    price_selector: None,
                }],
            }],
        };

        assert!(config.browser.cdp_url.is_none());
    }
}
