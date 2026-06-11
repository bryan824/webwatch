use std::sync::Arc;

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
    config::{AppConfig, Condition, TargetStatus},
    db,
    db::Persistence,
    discord,
    scheduler::Scheduler,
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
    conditions: Vec<Condition>,
}

impl From<CreateTargetRequest> for CreateTarget {
    fn from(request: CreateTargetRequest) -> Self {
        Self {
            name: request.name,
            url: request.url,
            enabled: request.enabled,
            interval_secs: request.interval_secs,
            conditions: request.conditions,
        }
    }
}

#[derive(Debug, Deserialize)]
struct PatchTargetRequest {
    enabled: bool,
}

pub fn router(state: HttpState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/targets", get(targets).post(create_target))
        .route(
            "/targets/:id",
            get(static_handler)
                .delete(delete_target)
                .patch(patch_target),
        )
        .route("/targets/:id/status", get(target_status))
        .route("/notify/status", post(notify_status))
        .route("/targets/reload", post(reload_targets))
        .fallback(static_handler)
        .with_state(state)
        .layer(TraceLayer::new_for_http())
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        persistence_backend: db::backend_name(),
    })
}

fn lifecycle(state: &HttpState) -> TargetLifecycle {
    TargetLifecycle::new(state.db.clone(), state.scheduler.clone())
}

async fn targets(State(state): State<HttpState>) -> impl IntoResponse {
    match lifecycle(&state).statuses().await {
        Ok(statuses) => (StatusCode::OK, Json(statuses)).into_response(),
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

async fn reload_targets(State(state): State<HttpState>) -> impl IntoResponse {
    match lifecycle(&state).reload_from_config(&state.config).await {
        Ok(report) => (StatusCode::OK, Json(ReloadTargetsResponse::from(report))).into_response(),
        Err(error @ crate::Error::ReadTargets { .. })
        | Err(error @ crate::Error::ParseConfig { .. }) => bad_request(error),
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
        | Err(error @ crate::Error::InvalidSelector { .. }) => bad_request(error),
        Err(error) => internal_error(error),
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
