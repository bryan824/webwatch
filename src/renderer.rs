use std::{sync::Arc, time::Duration};

use base64::Engine as _;
use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use serde_json::{json, Value};
use tokio::sync::Semaphore;
use tokio_tungstenite::{connect_async, tungstenite::Message, WebSocketStream};
use url::Url;

use crate::{
    config::{AppConfig, RenderPlan, RenderScenario, RenderStep, RendererConfig},
    error::{MissingBrowserCdpUrlSnafu, Result},
};
use snafu::OptionExt;

#[derive(Debug, Clone)]
pub struct RenderRequest {
    pub target_id: String,
    pub url: String,
    pub plan: RenderPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedSnapshot {
    pub final_url: String,
    pub html: String,
    /// PNG screenshot of the page, captured best-effort (including error pages).
    pub screenshot_png: Option<Vec<u8>>,
    /// `Some` when navigation itself failed (e.g. `net::ERR_HTTP2_PROTOCOL_ERROR`
    /// or a `chrome-error://` landing). The shell must not treat such a page as a
    /// real result.
    pub nav_error: Option<String>,
    pub scenario_id: Option<String>,
    pub scenario_label: Option<String>,
    pub evidence: Vec<String>,
}

#[derive(Clone)]
pub struct RendererService {
    config: RendererConfig,
    endpoint: Option<String>,
    semaphore: Arc<Semaphore>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RendererDiagnostics {
    pub enabled: bool,
    pub configured: bool,
    pub ok: bool,
    pub message: String,
    pub checks: Vec<RendererDiagnosticCheck>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RendererDiagnosticCheck {
    pub name: &'static str,
    pub ok: bool,
    pub message: String,
}

impl RendererService {
    pub fn from_config(config: &AppConfig) -> Self {
        let endpoint = config
            .renderer
            .endpoint
            .clone()
            .or_else(|| config.browser.cdp_url.clone());
        Self::new(config.renderer.clone(), endpoint)
    }

    pub fn new(config: RendererConfig, endpoint: Option<String>) -> Self {
        let max_concurrency = config.max_concurrency.max(1);
        let endpoint = endpoint.or_else(|| config.endpoint.clone());
        Self {
            config,
            endpoint,
            semaphore: Arc::new(Semaphore::new(max_concurrency)),
        }
    }

    pub fn is_available(&self) -> bool {
        self.config.enabled && self.endpoint.is_some()
    }

    pub async fn diagnose(&self) -> RendererDiagnostics {
        let endpoint = self
            .endpoint
            .as_deref()
            .filter(|endpoint| !endpoint.trim().is_empty());
        let configured = self.config.enabled && endpoint.is_some();
        let mut checks = Vec::new();

        if !self.config.enabled {
            checks.push(RendererDiagnosticCheck {
                name: "renderer_config",
                ok: false,
                message: "renderer is disabled".to_string(),
            });
            return renderer_diagnostics(self.config.enabled, configured, checks);
        }

        let Some(endpoint) = endpoint else {
            checks.push(RendererDiagnosticCheck {
                name: "renderer_config",
                ok: false,
                message: "renderer endpoint is not configured".to_string(),
            });
            return renderer_diagnostics(self.config.enabled, configured, checks);
        };

        checks.push(RendererDiagnosticCheck {
            name: "renderer_config",
            ok: true,
            message: "renderer endpoint configured".to_string(),
        });

        let ws_url = if endpoint.starts_with("ws://") || endpoint.starts_with("wss://") {
            checks.push(RendererDiagnosticCheck {
                name: "discovery",
                ok: true,
                message: "using direct websocket endpoint".to_string(),
            });
            endpoint.to_string()
        } else {
            match discover_cdp_ws_url(endpoint, Some("webwatch-diagnostic")).await {
                Ok(ws_url) => {
                    checks.push(RendererDiagnosticCheck {
                        name: "discovery",
                        ok: true,
                        message: "CDP discovery returned a websocket endpoint".to_string(),
                    });
                    ws_url
                }
                Err(error) => {
                    checks.push(RendererDiagnosticCheck {
                        name: "discovery",
                        ok: false,
                        message: redact_diagnostic_error(&error),
                    });
                    return renderer_diagnostics(self.config.enabled, configured, checks);
                }
            }
        };

        let mut client = match CdpClient::connect(&ws_url, self.operation_timeout()).await {
            Ok(client) => {
                checks.push(RendererDiagnosticCheck {
                    name: "websocket",
                    ok: true,
                    message: "connected to CDP websocket".to_string(),
                });
                client
            }
            Err(error) => {
                checks.push(RendererDiagnosticCheck {
                    name: "websocket",
                    ok: false,
                    message: redact_diagnostic_error(&error),
                });
                return renderer_diagnostics(self.config.enabled, configured, checks);
            }
        };

        match client
            .browser_command("Browser.getVersion", json!({}))
            .await
        {
            Ok(result) => {
                let product = result
                    .get("result")
                    .and_then(|result| result.get("product"))
                    .and_then(Value::as_str)
                    .unwrap_or("Browser.getVersion returned");
                checks.push(RendererDiagnosticCheck {
                    name: "browser_version",
                    ok: true,
                    message: product.to_string(),
                });
            }
            Err(error) => checks.push(RendererDiagnosticCheck {
                name: "browser_version",
                ok: false,
                message: redact_diagnostic_error(&error),
            }),
        }
        let _ = client.close_attached_target().await;

        renderer_diagnostics(self.config.enabled, configured, checks)
    }

    pub async fn render(&self, request: RenderRequest) -> Result<RenderedSnapshot> {
        self.render_snapshots(request)
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| {
                renderer_error(
                    "response_missing",
                    "renderer produced no snapshots".to_string(),
                )
            })
    }

    pub async fn render_snapshots(&self, request: RenderRequest) -> Result<Vec<RenderedSnapshot>> {
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|error| renderer_error("concurrency", error.to_string()))?;
        let endpoint = self
            .endpoint
            .as_deref()
            .context(MissingBrowserCdpUrlSnafu)?;
        let ws_url =
            discover_cdp_ws_url(endpoint, request.plan.fingerprint_seed.as_deref()).await?;
        let mut client = CdpClient::connect(&ws_url, self.operation_timeout()).await?;
        let result: Result<Vec<RenderedSnapshot>> = async {
            client.command("Page.enable", json!({})).await?;
            client.command("Runtime.enable", json!({})).await?;

            if request.plan.scenarios.is_empty() {
                return Ok(vec![self.render_one(&mut client, &request, None).await?]);
            }

            let mut snapshots = Vec::with_capacity(request.plan.scenarios.len());
            for scenario in request.plan.scenarios.clone() {
                snapshots.push(
                    self.render_one(&mut client, &request, Some(&scenario))
                        .await?,
                );
            }
            Ok(snapshots)
        }
        .await;
        let _ = client.close_attached_target().await;
        result
    }

    async fn render_one(
        &self,
        client: &mut CdpClient,
        request: &RenderRequest,
        scenario: Option<&RenderScenario>,
    ) -> Result<RenderedSnapshot> {
        let mut evidence = vec![format!("rendered via CDP for target {}", request.target_id)];
        let mut nav_error: Option<String> = None;
        match client
            .command_with_timeout(
                "Page.navigate",
                json!({ "url": request.url }),
                self.navigation_timeout(),
            )
            .await
        {
            Ok(result) => {
                // `Page.navigate` succeeds at the protocol level but reports
                // `errorText` when the load itself failed (bot blocking, TLS,
                // HTTP/2). The resulting page is Chrome's error screen, not the
                // site, so conditions must not be evaluated against it.
                if let Some(error_text) = result
                    .get("result")
                    .and_then(|result| result.get("errorText"))
                    .and_then(Value::as_str)
                {
                    nav_error = Some(error_text.to_string());
                    evidence.push(format!("navigation failed: {error_text}"));
                }
            }
            Err(error) => {
                let has_followup_steps = !request.plan.steps.is_empty()
                    || scenario
                        .map(|scenario| !scenario.steps.is_empty())
                        .unwrap_or(false);
                if !has_followup_steps {
                    return Err(error);
                }
                evidence.push(format!(
                    "Page.navigate did not finish before timeout; continuing with bounded render steps: {error}"
                ));
                match client
                    .command_with_timeout("Page.stopLoading", json!({}), Duration::from_secs(2))
                    .await
                {
                    Ok(_) => evidence
                        .push("stopped pending page load after navigation timeout".to_string()),
                    Err(stop_error) => evidence.push(format!(
                        "could not stop pending page load after navigation timeout: {stop_error}"
                    )),
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(
            request.plan.wait_ms.unwrap_or(self.config.settle_ms),
        ))
        .await;

        for step in &request.plan.steps {
            evidence.push(self.execute_step(client, step).await?);
        }
        if let Some(scenario) = scenario {
            evidence.push(format!(
                "render scenario {} ({})",
                scenario.id, scenario.label
            ));
            for step in &scenario.steps {
                evidence.push(self.execute_step(client, step).await?);
            }
        }

        let result = client
            .command(
                "Runtime.evaluate",
                json!({
                    "expression": "document.documentElement.outerHTML",
                    "returnByValue": true,
                }),
            )
            .await?;
        let html = runtime_string_result(&result, "Runtime.evaluate")?;
        let final_url = client
            .command(
                "Runtime.evaluate",
                json!({
                    "expression": "location.href",
                    "returnByValue": true,
                }),
            )
            .await
            .ok()
            .and_then(|value| runtime_string_result(&value, "Runtime.evaluate").ok())
            .unwrap_or_default();
        if nav_error.is_none() && final_url.starts_with("chrome-error://") {
            nav_error = Some(format!("loaded a browser error page ({final_url})"));
        }
        let screenshot_png = self.capture_screenshot(client).await;

        Ok(RenderedSnapshot {
            final_url,
            html,
            screenshot_png,
            nav_error,
            scenario_id: scenario.map(|scenario| scenario.id.clone()),
            scenario_label: scenario.map(|scenario| scenario.label.clone()),
            evidence,
        })
    }

    /// Best-effort PNG screenshot of the current page. Returns `None` if the
    /// browser declines (e.g. an error page) rather than failing the render.
    async fn capture_screenshot(&self, client: &mut CdpClient) -> Option<Vec<u8>> {
        let result = client
            .command("Page.captureScreenshot", json!({ "format": "png" }))
            .await
            .ok()?;
        let data = result
            .get("result")
            .and_then(|result| result.get("data"))
            .and_then(Value::as_str)?;
        base64::engine::general_purpose::STANDARD.decode(data).ok()
    }

    async fn execute_step(&self, client: &mut CdpClient, step: &RenderStep) -> Result<String> {
        let mut step = step.clone();
        if step.timeout_ms.is_none() {
            step.timeout_ms = Some(self.config.operation_timeout_ms);
        }
        if step.settle_ms.is_none() {
            step.settle_ms = Some(self.config.settle_ms);
        }
        let timeout_ms = step.timeout_ms.unwrap_or(self.config.operation_timeout_ms);
        let settle_ms = step.settle_ms.unwrap_or(self.config.settle_ms);
        let step_json = serde_json::to_string(&step)
            .map_err(|error| renderer_error("step", format!("serialize render step: {error}")))?;
        let expression = format!("({RENDER_STEP_SCRIPT})({step_json})");
        let result = client
            .command_with_timeout(
                "Runtime.evaluate",
                json!({
                    "expression": expression,
                    "awaitPromise": true,
                    "returnByValue": true,
                }),
                Duration::from_millis(timeout_ms.saturating_add(settle_ms).saturating_add(500)),
            )
            .await?;
        runtime_string_result(&result, "Runtime.evaluate")
    }

    fn operation_timeout(&self) -> Duration {
        Duration::from_millis(self.config.operation_timeout_ms)
    }

    fn navigation_timeout(&self) -> Duration {
        Duration::from_millis(self.config.navigation_timeout_ms)
    }
}

async fn discover_cdp_ws_url(endpoint: &str, fingerprint_seed: Option<&str>) -> Result<String> {
    if endpoint.starts_with("ws://") || endpoint.starts_with("wss://") {
        return Ok(endpoint.to_string());
    }

    let version_url = version_url(endpoint, fingerprint_seed)?;
    let response = reqwest::get(version_url.as_str())
        .await
        .map_err(|error| renderer_error("discover", format!("{version_url}: {error}")))?;
    let status = response.status();
    if !status.is_success() {
        return Err(renderer_error(
            "discover",
            format!("{version_url} returned HTTP {status}"),
        ));
    }
    let value: Value = response
        .json()
        .await
        .map_err(|error| renderer_error("discover", format!("{version_url}: {error}")))?;
    value
        .get("webSocketDebuggerUrl")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            renderer_error(
                "discover",
                "missing webSocketDebuggerUrl in /json/version".to_string(),
            )
        })
}

fn version_url(endpoint: &str, fingerprint_seed: Option<&str>) -> Result<Url> {
    let mut url = Url::parse(endpoint).map_err(|error| {
        renderer_error("discover", format!("invalid endpoint {endpoint}: {error}"))
    })?;
    match url.scheme() {
        "http" | "https" => {}
        scheme => {
            return Err(renderer_error(
                "discover",
                format!("unsupported CDP endpoint scheme {scheme}"),
            ));
        }
    }
    let query_pairs = url
        .query_pairs()
        .filter(|(key, _)| fingerprint_seed.is_none() || key != "fingerprint")
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();
    url.set_path("/json/version");
    url.set_query(None);
    {
        let mut pairs = url.query_pairs_mut();
        for (key, value) in query_pairs {
            pairs.append_pair(&key, &value);
        }
        if let Some(seed) = fingerprint_seed {
            pairs.append_pair("fingerprint", seed);
        }
    }
    Ok(url)
}

fn renderer_diagnostics(
    enabled: bool,
    configured: bool,
    checks: Vec<RendererDiagnosticCheck>,
) -> RendererDiagnostics {
    let ok = configured && checks.iter().all(|check| check.ok);
    let message = if ok {
        "renderer connection verified".to_string()
    } else if !enabled {
        "renderer is disabled".to_string()
    } else if !configured {
        "renderer is not configured".to_string()
    } else {
        checks
            .iter()
            .find(|check| !check.ok)
            .map(|check| check.message.clone())
            .unwrap_or_else(|| "renderer connection failed".to_string())
    };
    RendererDiagnostics {
        enabled,
        configured,
        ok,
        message,
        checks,
    }
}

fn redact_diagnostic_error(error: &crate::Error) -> String {
    redact_urls(&error.to_string())
}

fn redact_urls(value: &str) -> String {
    let schemes = ["http://", "https://", "ws://", "wss://"];
    let mut redacted = String::with_capacity(value.len());
    let mut index = 0;

    while index < value.len() {
        let rest = &value[index..];
        let Some((offset, _scheme)) = schemes
            .iter()
            .filter_map(|scheme| rest.find(scheme).map(|offset| (offset, *scheme)))
            .min_by_key(|(offset, _)| *offset)
        else {
            redacted.push_str(rest);
            break;
        };

        redacted.push_str(&rest[..offset]);
        redacted.push_str("<url>");

        let url_start = index + offset;
        let mut url_end = value.len();
        for (char_offset, character) in value[url_start..].char_indices().skip(1) {
            if character.is_whitespace() || matches!(character, '"' | '\'' | ')' | ']' | '}') {
                url_end = url_start + char_offset;
                break;
            }
        }
        index = url_end;
    }

    redacted
}

fn runtime_string_result(value: &Value, method: &'static str) -> Result<String> {
    if let Some(exception) = value
        .get("result")
        .and_then(|result| result.get("exceptionDetails"))
    {
        return Err(renderer_error("runtime", format!("{method}: {exception}")));
    }
    value
        .get("result")
        .and_then(|value| value.get("result"))
        .and_then(|value| value.get("value"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            renderer_error(
                "response_missing",
                format!("{method} missing result.result.value"),
            )
        })
}

fn renderer_error(stage: &'static str, message: String) -> crate::Error {
    crate::Error::Browser { stage, message }
}

const RENDER_STEP_SCRIPT: &str = r#"async function(step) {
    const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
    const textOf = (element) => (element.innerText || element.textContent || '').replace(/\s+/g, ' ').trim();
    const includesText = (value, needle) => value.toLowerCase().includes(String(needle).toLowerCase());
    const settle = async () => {
        if (step.settle_ms && step.settle_ms > 0) {
            await sleep(step.settle_ms);
        }
    };
    const deadline = () => Date.now() + (step.timeout_ms || 1000);
    const waitUntil = async (predicate, description) => {
        const end = deadline();
        do {
            const value = predicate();
            if (value) return value;
            await sleep(100);
        } while (Date.now() <= end);
        throw new Error(`Timed out waiting for ${description}`);
    };
    const elements = () => Array.from(document.querySelectorAll(step.selector || ''));
    const filtered = () => {
        const all = elements();
        if (!step.text) return all;
        return all.filter((element) => includesText(textOf(element), step.text));
    };

    switch (step.op) {
        case 'wait_for': {
            await waitUntil(() => elements().length > 0, `selector ${step.selector}`);
            return `wait_for selector '${step.selector}' matched`;
        }
        case 'wait_for_text': {
            await waitUntil(() => includesText(textOf(document.body || document.documentElement), step.text), `text ${step.text}`);
            return `wait_for_text '${step.text}' matched`;
        }
        case 'click': {
            const element = await waitUntil(() => filtered()[0], `click target ${step.selector}`);
            element.scrollIntoView({block: 'center', inline: 'center'});
            element.click();
            await settle();
            return `clicked selector '${step.selector}'`;
        }
        case 'select': {
            const element = await waitUntil(() => elements()[0], `select ${step.selector}`);
            if (!(element instanceof HTMLSelectElement)) {
                throw new Error(`Selector ${step.selector} did not match a select element`);
            }
            const option = Array.from(element.options).find((candidate) => {
                if (step.option_value !== undefined && step.option_value !== null) {
                    return candidate.value === step.option_value;
                }
                return textOf(candidate) === step.option_text;
            });
            if (!option) {
                throw new Error(`Option not found for ${step.selector}`);
            }
            element.value = option.value;
            option.selected = true;
            element.dispatchEvent(new Event('input', {bubbles: true}));
            element.dispatchEvent(new Event('change', {bubbles: true}));
            await settle();
            return `selected option '${textOf(option)}' for selector '${step.selector}'`;
        }
        default:
            throw new Error(`Unsupported render step op ${step.op}`);
    }
}"#;

type MaybeTlsStream = tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>;

struct CdpClient {
    stream: WebSocketStream<MaybeTlsStream>,
    next_id: u64,
    timeout: Duration,
    session_id: Option<String>,
    target_id: Option<String>,
}

impl CdpClient {
    async fn connect(ws_url: &str, timeout: Duration) -> Result<Self> {
        let connect = connect_async(ws_url);
        let (stream, _) = tokio::time::timeout(timeout, connect)
            .await
            .map_err(|_| renderer_error("connect", format!("timed out connecting to {ws_url}")))?
            .map_err(|error| renderer_error("connect", format!("{ws_url}: {error}")))?;
        let mut client = Self {
            stream,
            next_id: 1,
            timeout,
            session_id: None,
            target_id: None,
        };
        if ws_url.contains("/devtools/browser/") {
            client.attach_to_new_page().await?;
        }
        Ok(client)
    }

    async fn attach_to_new_page(&mut self) -> Result<()> {
        let target = self
            .browser_command("Target.createTarget", json!({ "url": "about:blank" }))
            .await?;
        let target_id = target
            .get("result")
            .and_then(|result| result.get("targetId"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| {
                renderer_error(
                    "response_missing",
                    "Target.createTarget missing result.targetId".to_string(),
                )
            })?;
        let attached = self
            .browser_command(
                "Target.attachToTarget",
                json!({ "targetId": target_id, "flatten": true }),
            )
            .await?;
        let session_id = attached
            .get("result")
            .and_then(|result| result.get("sessionId"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| {
                renderer_error(
                    "response_missing",
                    "Target.attachToTarget missing result.sessionId".to_string(),
                )
            })?;
        self.target_id = Some(target_id);
        self.session_id = Some(session_id);
        Ok(())
    }

    async fn command(&mut self, method: &'static str, params: Value) -> Result<Value> {
        self.command_with_timeout(method, params, self.timeout)
            .await
    }

    async fn command_with_timeout(
        &mut self,
        method: &'static str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value> {
        let session_id = self.session_id.clone();
        tokio::time::timeout(
            timeout,
            self.command_inner(method, params, session_id.as_deref()),
        )
        .await
        .map_err(|_| renderer_error("timeout", format!("{method} timed out")))?
    }

    async fn browser_command(&mut self, method: &'static str, params: Value) -> Result<Value> {
        tokio::time::timeout(self.timeout, self.command_inner(method, params, None))
            .await
            .map_err(|_| renderer_error("timeout", format!("{method} timed out")))?
    }

    async fn close_attached_target(&mut self) -> Result<()> {
        let Some(target_id) = self.target_id.take() else {
            return Ok(());
        };
        self.session_id = None;
        self.browser_command("Target.closeTarget", json!({ "targetId": target_id }))
            .await?;
        Ok(())
    }

    async fn command_inner(
        &mut self,
        method: &'static str,
        params: Value,
        session_id: Option<&str>,
    ) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        let mut message = json!({
            "id": id,
            "method": method,
            "params": params,
        });
        if let Some(session_id) = session_id {
            message["sessionId"] = Value::String(session_id.to_string());
        }
        self.stream
            .send(Message::Text(message.to_string().into()))
            .await
            .map_err(|error| renderer_error("send", format!("{method}: {error}")))?;

        while let Some(message) = self.stream.next().await {
            let message =
                message.map_err(|error| renderer_error("read", format!("{method}: {error}")))?;
            let Message::Text(text) = message else {
                continue;
            };
            let value: Value = serde_json::from_str(&text).map_err(|error| {
                renderer_error(
                    "protocol",
                    format!("{method}: invalid JSON response: {error}"),
                )
            })?;
            if value.get("id").and_then(Value::as_u64) != Some(id) {
                continue;
            }
            if let Some(error) = value.get("error") {
                return Err(renderer_error("protocol", format!("{method}: {error}")));
            }
            return Ok(value);
        }

        Err(renderer_error(
            "response_missing",
            format!("{method} missing response"),
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use futures_util::{SinkExt, StreamExt};
    use serde_json::Value;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio_tungstenite::{accept_async, tungstenite::Message};

    use super::{discover_cdp_ws_url, RendererService};
    use crate::config::{RenderPlan, RendererConfig};
    use crate::renderer::RenderRequest;

    #[tokio::test]
    async fn discover_preserves_cloakserve_query_params() {
        let seen_request = Arc::new(Mutex::new(String::new()));
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let seen = seen_request.clone();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept");
            let mut buf = [0; 2048];
            let n = stream.read(&mut buf).await.expect("read");
            let request = String::from_utf8_lossy(&buf[..n]).to_string();
            *seen.lock().expect("lock") = request.lines().next().unwrap_or_default().to_string();
            let body = r#"{"webSocketDebuggerUrl":"ws://127.0.0.1:1/devtools/browser/test"}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).await.expect("write");
        });

        let url = discover_cdp_ws_url(&format!("http://{addr}?fingerprint=abc&geoip=true"), None)
            .await
            .expect("discover");

        assert_eq!(url, "ws://127.0.0.1:1/devtools/browser/test");
        assert_eq!(
            seen_request.lock().expect("lock").as_str(),
            "GET /json/version?fingerprint=abc&geoip=true HTTP/1.1"
        );
    }

    #[tokio::test]
    async fn target_fingerprint_seed_overrides_endpoint_query() {
        let seen_request = Arc::new(Mutex::new(String::new()));
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let seen = seen_request.clone();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept");
            let mut buf = [0; 2048];
            let n = stream.read(&mut buf).await.expect("read");
            let request = String::from_utf8_lossy(&buf[..n]).to_string();
            *seen.lock().expect("lock") = request.lines().next().unwrap_or_default().to_string();
            let body = r#"{"webSocketDebuggerUrl":"ws://127.0.0.1:1/devtools/browser/test"}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).await.expect("write");
        });

        let url = discover_cdp_ws_url(
            &format!("http://{addr}?fingerprint=global&geoip=true"),
            Some("target-seed"),
        )
        .await
        .expect("discover");

        assert_eq!(url, "ws://127.0.0.1:1/devtools/browser/test");
        assert_eq!(
            seen_request.lock().expect("lock").as_str(),
            "GET /json/version?geoip=true&fingerprint=target-seed HTTP/1.1"
        );
    }

    #[tokio::test]
    async fn direct_websocket_endpoint_skips_discovery() {
        let endpoint = "ws://127.0.0.1:9222/devtools/browser/test";
        assert_eq!(
            discover_cdp_ws_url(endpoint, Some("target-seed"))
                .await
                .expect("discover"),
            endpoint
        );
    }

    #[tokio::test]
    async fn renderer_extracts_html_after_events_before_responses() {
        let ws_listener = TcpListener::bind("127.0.0.1:0").await.expect("bind ws");
        let ws_addr = ws_listener.local_addr().expect("ws addr");
        tokio::spawn(async move {
            let (stream, _) = ws_listener.accept().await.expect("accept ws");
            let mut ws = accept_async(stream).await.expect("handshake");
            while let Some(message) = ws.next().await {
                let Message::Text(text) = message.expect("message") else {
                    continue;
                };
                let value: Value = serde_json::from_str(&text).expect("json");
                let id = value.get("id").and_then(Value::as_u64).expect("id");
                let method = value.get("method").and_then(Value::as_str).expect("method");
                ws.send(Message::Text(
                    r#"{"method":"Runtime.consoleAPICalled","params":{}}"#.into(),
                ))
                .await
                .expect("send event");
                let response = match method {
                    "Runtime.evaluate" => {
                        let expression = value
                            .get("params")
                            .and_then(|params| params.get("expression"))
                            .and_then(Value::as_str)
                            .unwrap_or_default();
                        let result = if expression == "location.href" {
                            "https://example.com/final"
                        } else {
                            "<html><body>Rendered</body></html>"
                        };
                        serde_json::json!({"id": id, "result": {"result": {"value": result}}})
                    }
                    _ => serde_json::json!({"id": id, "result": {}}),
                };
                ws.send(Message::Text(response.to_string().into()))
                    .await
                    .expect("send response");
            }
        });

        let renderer = RendererService::new(
            RendererConfig {
                enabled: true,
                endpoint: Some(format!("ws://{ws_addr}")),
                settle_ms: 0,
                operation_timeout_ms: 1000,
                ..RendererConfig::default()
            },
            Some(format!("ws://{ws_addr}")),
        );
        let snapshot = renderer
            .render(RenderRequest {
                target_id: "target".to_string(),
                url: "https://example.com".to_string(),
                plan: RenderPlan::default(),
            })
            .await
            .expect("render");

        assert_eq!(snapshot.html, "<html><body>Rendered</body></html>");
        assert_eq!(snapshot.final_url, "https://example.com/final");
    }

    #[tokio::test]
    async fn renderer_attaches_to_browser_websocket_before_page_commands() {
        let ws_listener = TcpListener::bind("127.0.0.1:0").await.expect("bind ws");
        let ws_addr = ws_listener.local_addr().expect("ws addr");
        tokio::spawn(async move {
            let (stream, _) = ws_listener.accept().await.expect("accept ws");
            let mut ws = accept_async(stream).await.expect("handshake");
            while let Some(message) = ws.next().await {
                let Message::Text(text) = message.expect("message") else {
                    continue;
                };
                let value: Value = serde_json::from_str(&text).expect("json");
                let id = value.get("id").and_then(Value::as_u64).expect("id");
                let method = value.get("method").and_then(Value::as_str).expect("method");
                let response = match method {
                    "Target.createTarget" => {
                        serde_json::json!({"id": id, "result": {"targetId": "target-1"}})
                    }
                    "Target.attachToTarget" => {
                        serde_json::json!({"id": id, "result": {"sessionId": "session-1"}})
                    }
                    "Target.closeTarget" => {
                        serde_json::json!({"id": id, "result": {"success": true}})
                    }
                    "Runtime.evaluate" => {
                        assert_eq!(
                            value.get("sessionId").and_then(Value::as_str),
                            Some("session-1")
                        );
                        let expression = value
                            .get("params")
                            .and_then(|params| params.get("expression"))
                            .and_then(Value::as_str)
                            .unwrap_or_default();
                        let result = if expression == "location.href" {
                            "https://example.com/final"
                        } else {
                            "<html><body>Rendered from session</body></html>"
                        };
                        serde_json::json!({"id": id, "sessionId": "session-1", "result": {"result": {"value": result}}})
                    }
                    _ => {
                        assert_eq!(
                            value.get("sessionId").and_then(Value::as_str),
                            Some("session-1")
                        );
                        serde_json::json!({"id": id, "sessionId": "session-1", "result": {}})
                    }
                };
                ws.send(Message::Text(response.to_string().into()))
                    .await
                    .expect("send response");
            }
        });

        let renderer = RendererService::new(
            RendererConfig {
                enabled: true,
                endpoint: Some(format!("ws://{ws_addr}/devtools/browser/test")),
                settle_ms: 0,
                operation_timeout_ms: 1000,
                ..RendererConfig::default()
            },
            None,
        );
        let snapshot = renderer
            .render(RenderRequest {
                target_id: "target".to_string(),
                url: "https://example.com".to_string(),
                plan: RenderPlan::default(),
            })
            .await
            .expect("render");

        assert_eq!(
            snapshot.html,
            "<html><body>Rendered from session</body></html>"
        );
        assert_eq!(snapshot.final_url, "https://example.com/final");
    }

    #[tokio::test]
    async fn renderer_captures_screenshot_and_navigation_error() {
        let ws_listener = TcpListener::bind("127.0.0.1:0").await.expect("bind ws");
        let ws_addr = ws_listener.local_addr().expect("ws addr");
        tokio::spawn(async move {
            let (stream, _) = ws_listener.accept().await.expect("accept ws");
            let mut ws = accept_async(stream).await.expect("handshake");
            while let Some(message) = ws.next().await {
                let Message::Text(text) = message.expect("message") else {
                    continue;
                };
                let value: Value = serde_json::from_str(&text).expect("json");
                let id = value.get("id").and_then(Value::as_u64).expect("id");
                let method = value.get("method").and_then(Value::as_str).expect("method");
                let response = match method {
                    "Page.navigate" => {
                        serde_json::json!({"id": id, "result": {"errorText": "net::ERR_FAILED"}})
                    }
                    // base64 "AQID" decodes to the bytes [1, 2, 3]
                    "Page.captureScreenshot" => {
                        serde_json::json!({"id": id, "result": {"data": "AQID"}})
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
                            "<html><body>error page</body></html>"
                        };
                        serde_json::json!({"id": id, "result": {"result": {"value": result}}})
                    }
                    _ => serde_json::json!({"id": id, "result": {}}),
                };
                ws.send(Message::Text(response.to_string().into()))
                    .await
                    .expect("send response");
            }
        });

        let renderer = RendererService::new(
            RendererConfig {
                enabled: true,
                endpoint: Some(format!("ws://{ws_addr}")),
                settle_ms: 0,
                operation_timeout_ms: 1000,
                ..RendererConfig::default()
            },
            Some(format!("ws://{ws_addr}")),
        );
        let snapshot = renderer
            .render(RenderRequest {
                target_id: "target".to_string(),
                url: "https://example.com".to_string(),
                plan: RenderPlan::default(),
            })
            .await
            .expect("render");

        assert_eq!(snapshot.nav_error.as_deref(), Some("net::ERR_FAILED"));
        assert_eq!(snapshot.screenshot_png, Some(vec![1, 2, 3]));
    }

    #[tokio::test]
    async fn navigation_timeout_is_reported() {
        let ws_listener = TcpListener::bind("127.0.0.1:0").await.expect("bind ws");
        let ws_addr = ws_listener.local_addr().expect("ws addr");
        tokio::spawn(async move {
            let (stream, _) = ws_listener.accept().await.expect("accept ws");
            let mut ws = accept_async(stream).await.expect("handshake");
            while let Some(message) = ws.next().await {
                let Message::Text(text) = message.expect("message") else {
                    continue;
                };
                let value: Value = serde_json::from_str(&text).expect("json");
                let id = value.get("id").and_then(Value::as_u64).expect("id");
                let method = value.get("method").and_then(Value::as_str).expect("method");
                if method == "Page.navigate" {
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                    continue;
                }
                ws.send(Message::Text(
                    serde_json::json!({"id": id, "result": {}})
                        .to_string()
                        .into(),
                ))
                .await
                .expect("send response");
            }
        });

        let renderer = RendererService::new(
            RendererConfig {
                enabled: true,
                endpoint: Some(format!("ws://{ws_addr}")),
                operation_timeout_ms: 1000,
                navigation_timeout_ms: 10,
                settle_ms: 0,
                ..RendererConfig::default()
            },
            None,
        );
        let error = renderer
            .render(RenderRequest {
                target_id: "target".to_string(),
                url: "https://example.com".to_string(),
                plan: RenderPlan::default(),
            })
            .await
            .expect_err("timeout");

        assert!(error.to_string().contains("Page.navigate timed out"));
    }

    #[tokio::test]
    async fn cdp_command_timeout_is_reported() {
        let ws_listener = TcpListener::bind("127.0.0.1:0").await.expect("bind ws");
        let ws_addr = ws_listener.local_addr().expect("ws addr");
        tokio::spawn(async move {
            let (stream, _) = ws_listener.accept().await.expect("accept ws");
            let mut ws = accept_async(stream).await.expect("handshake");
            let _ = ws.next().await;
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        });

        let renderer = RendererService::new(
            RendererConfig {
                enabled: true,
                endpoint: Some(format!("ws://{ws_addr}")),
                operation_timeout_ms: 10,
                settle_ms: 0,
                ..RendererConfig::default()
            },
            None,
        );
        let error = renderer
            .render(RenderRequest {
                target_id: "target".to_string(),
                url: "https://example.com".to_string(),
                plan: RenderPlan::default(),
            })
            .await
            .expect_err("timeout");

        assert!(error.to_string().contains("Page.enable timed out"));
    }
}
