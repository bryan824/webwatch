use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use snafu::OptionExt;
use tokio_tungstenite::{connect_async, tungstenite::Message, WebSocketStream};

use crate::{
    config::AppConfig,
    config::{CheckOutcome, EngineUsed, Target},
    error::{MissingBrowserCdpUrlSnafu, Result},
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
    let (stream, _) = connect_async(cdp_url)
        .await
        .map_err(|error| browser_error("connect", format!("{cdp_url}: {error}")))?;
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
        .ok_or_else(|| {
            browser_error(
                "response_missing",
                "Runtime.evaluate missing result.result.value".to_string(),
            )
        })
}

fn browser_error(stage: &'static str, message: String) -> crate::Error {
    crate::Error::Browser { stage, message }
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
            .map_err(|error| browser_error("send", format!("{method}: {error}")))?;

        while let Some(message) = self.stream.next().await {
            let message =
                message.map_err(|error| browser_error("read", format!("{method}: {error}")))?;
            let Message::Text(text) = message else {
                continue;
            };
            let value: Value = serde_json::from_str(&text).map_err(|error| {
                browser_error(
                    "protocol",
                    format!("{method}: invalid JSON response: {error}"),
                )
            })?;
            if value.get("id").and_then(Value::as_u64) != Some(id) {
                continue;
            }
            if let Some(error) = value.get("error") {
                return Err(browser_error("protocol", format!("{method}: {error}")));
            }
            return Ok(value);
        }

        Err(browser_error(
            "response_missing",
            format!("{method} missing response"),
        ))
    }
}

#[cfg(test)]
mod tests {
    use crate::config::{AppConfig, BrowserConfig, SchedulerConfig, ServerConfig};

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
        };

        assert!(config.browser.cdp_url.is_none());
    }
}
