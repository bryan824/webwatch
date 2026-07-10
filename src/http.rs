use std::{sync::Arc, time::Instant};

use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use tower_http::trace::TraceLayer;

#[derive(RustEmbed)]
#[folder = "web/build"]
struct WebAssets;

async fn static_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let candidate = if path.is_empty() { "index.html" } else { path };

    if let Some(content) = WebAssets::get(candidate) {
        let mime = mime_guess::from_path(candidate).first_or_octet_stream();
        return Response::builder()
            .header(header::CONTENT_TYPE, mime.as_ref())
            .body(Body::from(content.data.into_owned()))
            .unwrap();
    }
    // SPA fallback: serve index.html for client-side routes
    match WebAssets::get("index.html") {
        Some(content) => Response::builder()
            .header(header::CONTENT_TYPE, "text/html")
            .body(Body::from(content.data.into_owned()))
            .unwrap(),
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("frontend not built"))
            .unwrap(),
    }
}

use crate::{
    config::{AppConfig, Condition, EngineUsed, RenderPlan, RendererBackend, Target, TargetStatus},
    db,
    db::Persistence,
    discord,
    scheduler::{Scheduler, SchedulerStats},
    targets::{CreateTarget, ReloadReport, TargetLifecycle},
};

#[derive(Clone)]
pub struct HttpState {
    pub config: Arc<AppConfig>,
    pub scheduler: Arc<Scheduler>,
    pub db: Arc<dyn Persistence>,
    pub client: reqwest::Client,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
    persistence_backend: &'static str,
    discord_configured: bool,
    renderer_enabled: bool,
    renderer_configured: bool,
    renderer_backend: RendererBackend,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Debug, Serialize)]
struct NotifyStatusResponse {
    sent: bool,
    summary: String,
    statuses: Vec<TargetStatus>,
}

#[derive(Debug, Serialize)]
struct ReloadTargetsResponse {
    added: Vec<String>,
    removed: Vec<String>,
    changed: Vec<String>,
    unchanged: Vec<String>,
}

impl From<ReloadReport> for ReloadTargetsResponse {
    fn from(report: ReloadReport) -> Self {
        Self {
            added: report.added,
            removed: report.removed,
            changed: report.changed,
            unchanged: report.unchanged,
        }
    }
}

#[derive(Debug, Deserialize)]
struct CreateTargetRequest {
    name: String,
    url: String,
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    interval_secs: Option<u64>,
    #[serde(default)]
    render: RenderPlan,
    #[serde(default)]
    conditions: Vec<Condition>,
}

impl From<CreateTargetRequest> for CreateTarget {
    fn from(request: CreateTargetRequest) -> Self {
        Self {
            name: request.name,
            url: request.url,
            enabled: request.enabled,
            interval_secs: request.interval_secs,
            render: request.render,
            conditions: request.conditions,
        }
    }
}

#[derive(Debug, Deserialize)]
struct PatchTargetRequest {
    enabled: bool,
}

#[derive(Debug, Serialize)]
struct TargetDetailResponse {
    config: Target,
    status: TargetStatus,
}

#[derive(Debug, Deserialize)]
struct DryRunRequest {
    #[serde(default)]
    target_id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    url: String,
    #[serde(default)]
    render: RenderPlan,
    #[serde(default)]
    conditions: Vec<Condition>,
}

#[derive(Debug, Serialize)]
struct DryRunDiagnostic {
    kind: &'static str,
    message: String,
}

#[derive(Debug, Serialize)]
struct DryRunArtifacts {
    html_url: Option<String>,
    screenshot_url: Option<String>,
}

#[derive(Debug, Serialize)]
struct DryRunResponse {
    matched: Option<bool>,
    engine_used: Option<EngineUsed>,
    duration_ms: u128,
    final_url: Option<String>,
    evidence: Vec<String>,
    condition_results: Vec<crate::config::ConditionResult>,
    diagnostics: Vec<DryRunDiagnostic>,
    artifacts: DryRunArtifacts,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct OpsTargetCounts {
    total: usize,
    enabled: usize,
    matched: usize,
    no_match: usize,
    error: usize,
    unknown: usize,
    disabled: usize,
}

#[derive(Debug, Serialize)]
struct OpsRecentError {
    target_id: String,
    name: String,
    error: String,
    at: Option<String>,
}

#[derive(Debug, Serialize)]
struct OpsResponse {
    status: &'static str,
    persistence_backend: &'static str,
    discord_configured: bool,
    renderer_enabled: bool,
    renderer_configured: bool,
    renderer_available: bool,
    renderer_backend: RendererBackend,
    scheduler: SchedulerStats,
    targets: OpsTargetCounts,
    recent_errors: Vec<OpsRecentError>,
}

#[derive(Debug, Serialize)]
struct IntegrationTestResponse {
    configured: bool,
    ok: bool,
    message: String,
    checks: Vec<IntegrationCheck>,
}

#[derive(Debug, Serialize)]
struct IntegrationCheck {
    name: &'static str,
    ok: bool,
    message: String,
}

pub fn router(state: HttpState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/ops", get(ops_status))
        .route("/ops/discord/test", post(test_discord))
        .route("/ops/renderer/test", post(test_renderer))
        .route("/targets", get(targets).post(create_target))
        .route("/targets/dry-run", post(dry_run_target))
        .route(
            "/targets/:id",
            get(target_detail)
                .put(update_target)
                .delete(delete_target)
                .patch(patch_target),
        )
        .route("/targets/:id/checks", get(target_checks))
        .route("/targets/:id/dry-run", post(dry_run_existing_target))
        .route("/targets/:id/status", get(target_status))
        .route("/targets/:id/snapshot.html", get(snapshot_html))
        .route("/targets/:id/snapshot.png", get(snapshot_png))
        .route("/notify/status", post(notify_status))
        .route("/targets/export", get(export_targets))
        .route("/targets/import", post(import_targets))
        .fallback(static_handler)
        .with_state(state)
        .layer(TraceLayer::new_for_http())
}

async fn health(State(state): State<HttpState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        persistence_backend: db::backend_name(),
        discord_configured: discord_configured(&state.config),
        renderer_enabled: state.config.renderer.enabled,
        renderer_configured: state
            .config
            .renderer
            .endpoint
            .as_deref()
            .or(state.config.browser.cdp_url.as_deref())
            .is_some_and(|endpoint| !endpoint.trim().is_empty()),
        renderer_backend: state.config.renderer.backend,
    })
}

fn lifecycle(state: &HttpState) -> TargetLifecycle {
    TargetLifecycle::new(state.db.clone(), state.scheduler.clone())
}

fn discord_configured(config: &AppConfig) -> bool {
    config
        .discord_webhook_url
        .as_deref()
        .is_some_and(|url| !url.trim().is_empty())
}

async fn test_discord(State(state): State<HttpState>) -> Json<IntegrationTestResponse> {
    let configured = discord_configured(&state.config);
    let mut checks = vec![IntegrationCheck {
        name: "webhook_config",
        ok: configured,
        message: if configured {
            "Discord webhook configured".to_string()
        } else {
            "Discord webhook is not configured".to_string()
        },
    }];

    if !configured {
        return Json(integration_test_response(
            configured,
            "Discord webhook is not configured".to_string(),
            checks,
        ));
    }

    match discord::send_test_message(&state.client, &state.config).await {
        Ok(()) => checks.push(IntegrationCheck {
            name: "webhook_send",
            ok: true,
            message: "Discord accepted the test message".to_string(),
        }),
        Err(error) => checks.push(IntegrationCheck {
            name: "webhook_send",
            ok: false,
            message: safe_error_message(&error),
        }),
    }

    let message = if checks.iter().all(|check| check.ok) {
        "Discord webhook verified".to_string()
    } else {
        checks
            .iter()
            .find(|check| !check.ok)
            .map(|check| check.message.clone())
            .unwrap_or_else(|| "Discord webhook test failed".to_string())
    };

    Json(integration_test_response(configured, message, checks))
}

async fn test_renderer(State(state): State<HttpState>) -> impl IntoResponse {
    Json(state.scheduler.renderer().diagnose().await)
}

fn integration_test_response(
    configured: bool,
    message: String,
    checks: Vec<IntegrationCheck>,
) -> IntegrationTestResponse {
    IntegrationTestResponse {
        configured,
        ok: configured && checks.iter().all(|check| check.ok),
        message,
        checks,
    }
}

fn safe_error_message(error: &crate::Error) -> String {
    let message = match error {
        crate::Error::MissingDiscordWebhook => "Discord webhook is not configured".to_string(),
        crate::Error::DiscordStatus { status, body } => format!(
            "Discord returned HTTP {status}: {}",
            body.chars().take(240).collect::<String>()
        ),
        crate::Error::Request { source, .. } => format!("request failed: {source}"),
        other => other.to_string(),
    };
    redact_urls(&message)
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

async fn targets(State(state): State<HttpState>) -> impl IntoResponse {
    match lifecycle(&state).statuses().await {
        Ok(statuses) => (StatusCode::OK, Json(statuses)).into_response(),
        Err(error) => internal_error(error),
    }
}

async fn ops_status(State(state): State<HttpState>) -> impl IntoResponse {
    let scheduler = state.scheduler.stats().await;
    match lifecycle(&state).statuses().await {
        Ok(statuses) => {
            let targets = target_counts(&statuses);
            let recent_errors = statuses
                .iter()
                .filter_map(|status| {
                    status.last_error.as_ref().map(|error| OpsRecentError {
                        target_id: status.target_id.clone(),
                        name: status.name.clone(),
                        error: error.clone(),
                        at: status.last_error_at.clone(),
                    })
                })
                .take(10)
                .collect();
            (
                StatusCode::OK,
                Json(OpsResponse {
                    status: "ok",
                    persistence_backend: db::backend_name(),
                    discord_configured: discord_configured(&state.config),
                    renderer_enabled: state.config.renderer.enabled,
                    renderer_configured: state
                        .config
                        .renderer
                        .endpoint
                        .as_deref()
                        .or(state.config.browser.cdp_url.as_deref())
                        .is_some_and(|endpoint| !endpoint.trim().is_empty()),
                    renderer_available: scheduler.renderer_available,
                    renderer_backend: state.config.renderer.backend,
                    scheduler,
                    targets,
                    recent_errors,
                }),
            )
                .into_response()
        }
        Err(error) => internal_error(error),
    }
}

fn target_counts(statuses: &[TargetStatus]) -> OpsTargetCounts {
    OpsTargetCounts {
        total: statuses.len(),
        enabled: statuses.iter().filter(|status| status.enabled).count(),
        matched: statuses
            .iter()
            .filter(|status| status.matched == Some(true) && status.last_error.is_none())
            .count(),
        no_match: statuses
            .iter()
            .filter(|status| status.matched == Some(false) && status.last_error.is_none())
            .count(),
        error: statuses
            .iter()
            .filter(|status| status.last_error.is_some())
            .count(),
        unknown: statuses
            .iter()
            .filter(|status| status.matched.is_none() && status.last_error.is_none())
            .count(),
        disabled: statuses.iter().filter(|status| !status.enabled).count(),
    }
}

async fn target_detail(
    State(state): State<HttpState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let lifecycle = lifecycle(&state);
    match (lifecycle.config(&id).await, lifecycle.statuses().await) {
        (Ok(Some(config)), Ok(statuses)) => {
            let Some(status) = statuses.into_iter().find(|status| status.target_id == id) else {
                return internal_error(crate::Error::Database {
                    message: format!("target {id} missing status"),
                });
            };
            (
                StatusCode::OK,
                Json(TargetDetailResponse { config, status }),
            )
                .into_response()
        }
        (Ok(None), _) => not_found(&id),
        (Err(error), _) | (_, Err(error)) => internal_error(error),
    }
}

async fn target_checks(
    State(state): State<HttpState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.db.recent_checks(&id, 10).await {
        Ok(checks) => (StatusCode::OK, Json(checks)).into_response(),
        Err(error) => internal_error(error),
    }
}

async fn target_status(
    State(state): State<HttpState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match lifecycle(&state)
        .status(&state.config, &state.client, &id)
        .await
    {
        Ok(Some(status)) => (StatusCode::OK, Json(status)).into_response(),
        Ok(None) => not_found(&id),
        Err(error) => internal_error(error),
    }
}

async fn snapshot_html(State(state): State<HttpState>, Path(id): Path<String>) -> Response {
    serve_snapshot(&state, &id, "last.html", "text/html; charset=utf-8")
}

async fn snapshot_png(State(state): State<HttpState>, Path(id): Path<String>) -> Response {
    serve_snapshot(&state, &id, "last.png", "image/png")
}

fn serve_snapshot(state: &HttpState, id: &str, file: &str, content_type: &str) -> Response {
    if !is_safe_target_id(id) {
        return not_found(id);
    }
    let path = state.config.snapshot_dir(id).join(file);
    match std::fs::read(&path) {
        Ok(bytes) => Response::builder()
            .header(header::CONTENT_TYPE, content_type)
            .body(Body::from(bytes))
            .unwrap(),
        Err(_) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("no saved render for target {id}"),
            }),
        )
            .into_response(),
    }
}

/// Target ids are slugs; reject anything else so a snapshot path can never
/// escape the snapshots directory.
fn is_safe_target_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

async fn notify_status(State(state): State<HttpState>) -> impl IntoResponse {
    let lifecycle = lifecycle(&state);
    for target in state.scheduler.current_targets().await {
        match lifecycle
            .check_target_by_id(&state.config, &state.client, &target.id, true)
            .await
        {
            Ok(true) => {}
            Ok(false) => return not_found(&target.id),
            Err(error) => return internal_error(error),
        }
    }

    match lifecycle.statuses().await {
        Ok(statuses) => {
            let summary = discord::render_status_report(&statuses);
            match discord::send_status_report(&state.client, &state.config, &summary).await {
                Ok(()) => (
                    StatusCode::OK,
                    Json(NotifyStatusResponse {
                        sent: true,
                        summary,
                        statuses,
                    }),
                )
                    .into_response(),
                Err(error) => internal_error(error),
            }
        }
        Err(error) => internal_error(error),
    }
}

async fn export_targets(State(state): State<HttpState>) -> impl IntoResponse {
    match lifecycle(&state).export_toml().await {
        Ok(toml) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "application/toml; charset=utf-8"),
                (
                    header::CONTENT_DISPOSITION,
                    "attachment; filename=\"targets.toml\"",
                ),
            ],
            toml,
        )
            .into_response(),
        Err(error) => internal_error(error),
    }
}

async fn import_targets(State(state): State<HttpState>, body: String) -> impl IntoResponse {
    match lifecycle(&state).import_from_toml(&body).await {
        Ok(report) => (StatusCode::OK, Json(ReloadTargetsResponse::from(report))).into_response(),
        Err(error @ crate::Error::ParseConfig { .. })
        | Err(error @ crate::Error::EmptyConditions { .. })
        | Err(error @ crate::Error::ParseTargetUrl { .. })
        | Err(error @ crate::Error::MissingConditionField { .. })
        | Err(error @ crate::Error::InvalidSelector { .. })
        | Err(error @ crate::Error::InvalidRenderConfig { .. }) => bad_request(error),
        Err(error) => internal_error(error),
    }
}

async fn create_target(
    State(state): State<HttpState>,
    Json(request): Json<CreateTargetRequest>,
) -> impl IntoResponse {
    match lifecycle(&state).create(request.into()).await {
        Ok(status) => (StatusCode::CREATED, Json(status)).into_response(),
        Err(error @ crate::Error::EmptyConditions { .. })
        | Err(error @ crate::Error::ParseTargetUrl { .. })
        | Err(error @ crate::Error::MissingConditionField { .. })
        | Err(error @ crate::Error::InvalidSelector { .. })
        | Err(error @ crate::Error::InvalidRenderConfig { .. }) => bad_request(error),
        Err(error) => internal_error(error),
    }
}

async fn update_target(
    State(state): State<HttpState>,
    Path(id): Path<String>,
    Json(request): Json<CreateTargetRequest>,
) -> impl IntoResponse {
    match lifecycle(&state).update(&id, request.into()).await {
        Ok(Some(status)) => (StatusCode::OK, Json(status)).into_response(),
        Ok(None) => not_found(&id),
        Err(error @ crate::Error::EmptyConditions { .. })
        | Err(error @ crate::Error::ParseTargetUrl { .. })
        | Err(error @ crate::Error::MissingConditionField { .. })
        | Err(error @ crate::Error::InvalidSelector { .. })
        | Err(error @ crate::Error::InvalidRenderConfig { .. }) => bad_request(error),
        Err(error) => internal_error(error),
    }
}

async fn dry_run_target(
    State(state): State<HttpState>,
    Json(request): Json<DryRunRequest>,
) -> impl IntoResponse {
    run_dry_run(&state, request, None).await
}

async fn dry_run_existing_target(
    State(state): State<HttpState>,
    Path(id): Path<String>,
    Json(request): Json<DryRunRequest>,
) -> impl IntoResponse {
    run_dry_run(&state, request, Some(id)).await
}

async fn run_dry_run(
    state: &HttpState,
    request: DryRunRequest,
    path_id: Option<String>,
) -> axum::response::Response {
    let id = dry_run_id(path_id.as_deref().or(request.target_id.as_deref()));
    let target = Target {
        id: id.clone(),
        name: request.name.unwrap_or_else(|| "Dry run".to_string()),
        url: request.url,
        enabled: true,
        interval_secs: None,
        render: request.render,
        conditions: request.conditions,
    };
    let started = Instant::now();
    match lifecycle(state)
        .dry_run(&state.config, &state.client, target)
        .await
    {
        Ok(outcome) => {
            let artifacts = dry_run_artifacts(&id, Some(outcome.engine_used));
            (
                StatusCode::OK,
                Json(DryRunResponse {
                    matched: Some(outcome.matched),
                    engine_used: Some(outcome.engine_used),
                    duration_ms: started.elapsed().as_millis(),
                    final_url: Some(outcome.target.url.clone()),
                    evidence: outcome.evidence,
                    condition_results: outcome.condition_results,
                    diagnostics: Vec::new(),
                    artifacts,
                    error: None,
                }),
            )
                .into_response()
        }
        Err(error @ crate::Error::EmptyConditions { .. })
        | Err(error @ crate::Error::ParseTargetUrl { .. })
        | Err(error @ crate::Error::MissingConditionField { .. })
        | Err(error @ crate::Error::InvalidSelector { .. })
        | Err(error @ crate::Error::InvalidRenderConfig { .. }) => bad_request(error),
        Err(error) => {
            let message = error.to_string();
            (
                StatusCode::OK,
                Json(DryRunResponse {
                    matched: None,
                    engine_used: None,
                    duration_ms: started.elapsed().as_millis(),
                    final_url: None,
                    evidence: Vec::new(),
                    condition_results: Vec::new(),
                    diagnostics: vec![DryRunDiagnostic {
                        kind: classify_error(&error),
                        message: message.clone(),
                    }],
                    artifacts: dry_run_artifacts(&id, None),
                    error: Some(message),
                }),
            )
                .into_response()
        }
    }
}

fn dry_run_id(id: Option<&str>) -> String {
    let suffix = id.unwrap_or("draft");
    let safe = suffix
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    format!("dry-run-{safe}")
}

fn dry_run_artifacts(id: &str, engine: Option<EngineUsed>) -> DryRunArtifacts {
    if engine != Some(EngineUsed::BrowserCdp) {
        return DryRunArtifacts {
            html_url: None,
            screenshot_url: None,
        };
    }
    DryRunArtifacts {
        html_url: Some(format!("/targets/{id}/snapshot.html")),
        screenshot_url: Some(format!("/targets/{id}/snapshot.png")),
    }
}

fn classify_error(error: &crate::Error) -> &'static str {
    match error {
        crate::Error::Request { .. } => "network",
        crate::Error::HttpStatus { .. } => "http_status",
        crate::Error::BrowserRequired { .. } => "browser_required",
        crate::Error::MissingBrowserCdpUrl => "renderer_unavailable",
        crate::Error::Browser { stage, .. } => stage,
        crate::Error::InvalidSelector { .. } => "invalid_selector",
        crate::Error::MissingDiscordWebhook | crate::Error::DiscordStatus { .. } => "notification",
        _ => "error",
    }
}

async fn delete_target(
    State(state): State<HttpState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match lifecycle(&state).delete(&id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => not_found(&id),
        Err(error) => internal_error(error),
    }
}

async fn patch_target(
    State(state): State<HttpState>,
    Path(id): Path<String>,
    Json(request): Json<PatchTargetRequest>,
) -> impl IntoResponse {
    match lifecycle(&state).set_enabled(&id, request.enabled).await {
        Ok(Some(status)) => (StatusCode::OK, Json(status)).into_response(),
        Ok(None) => not_found(&id),
        Err(error) => internal_error(error),
    }
}

fn bad_request(error: crate::Error) -> axum::response::Response {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: error.to_string(),
        }),
    )
        .into_response()
}

fn not_found(id: &str) -> axum::response::Response {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse {
            error: format!("target {id} not found"),
        }),
    )
        .into_response()
}

fn internal_error(error: crate::Error) -> axum::response::Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            error: error.to_string(),
        }),
    )
        .into_response()
}
