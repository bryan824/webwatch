use std::{path::Path as FsPath, sync::Arc};

use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use rust_embed::RustEmbed;
use serde::Serialize;
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
    config::{AppConfig, TargetStatus, TargetsFile},
    db,
    db::Persistence,
    discord, evaluator,
    scheduler::{ReloadReport, Scheduler},
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

pub fn router(state: HttpState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/targets", get(targets))
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

async fn targets(State(state): State<HttpState>, headers: HeaderMap) -> impl IntoResponse {
    if let Some(response) = authorize_optional(&state, &headers) {
        return response;
    }

    match state.db.statuses().await {
        Ok(statuses) => (StatusCode::OK, Json(statuses)).into_response(),
        Err(error) => internal_error(error),
    }
}

async fn target_status(
    State(state): State<HttpState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if let Some(response) = authorize_optional(&state, &headers) {
        return response;
    }

    if let Err(response) = check_target_by_id(&state, &id, false).await {
        return response;
    }

    match state.db.status(&id).await {
        Ok(Some(status)) => (StatusCode::OK, Json(status)).into_response(),
        Ok(None) => not_found(&id),
        Err(error) => internal_error(error),
    }
}

async fn notify_status(State(state): State<HttpState>, headers: HeaderMap) -> impl IntoResponse {
    if let Some(response) = authorize_required(&state, &headers) {
        return response;
    }

    for target in state.scheduler.current_targets().await {
        if let Err(response) = check_target_by_id(&state, &target.id, true).await {
            return response;
        }
    }

    match state.db.statuses().await {
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

async fn reload_targets(State(state): State<HttpState>, headers: HeaderMap) -> impl IntoResponse {
    if let Some(response) = authorize_required(&state, &headers) {
        return response;
    }

    let Some(path) = state.config.targets_path.as_deref() else {
        return internal_error(crate::Error::Database {
            message: "targets_path not configured".to_string(),
        });
    };
    let targets = match TargetsFile::load(FsPath::new(path)) {
        Ok(targets) => targets,
        Err(error) => return bad_request(error),
    };

    match state.scheduler.reload(&targets.targets).await {
        Ok(report) => (StatusCode::OK, Json(ReloadTargetsResponse::from(report))).into_response(),
        Err(error) => internal_error(error),
    }
}

async fn check_target_by_id(
    state: &HttpState,
    id: &str,
    mark_manual_report: bool,
) -> Result<(), axum::response::Response> {
    let Some(target_config) = state.scheduler.target(id).await else {
        return Err(not_found(id));
    };

    match target_config.to_target() {
        Ok(target) => match evaluator::check_target(&state.config, &state.client, target).await {
            Ok(outcome) => match state.db.record_success(&outcome).await {
                Ok(should_alert) => {
                    if should_alert && mark_manual_report {
                        state
                            .db
                            .mark_alert_sent(&outcome.target.id)
                            .await
                            .map_err(internal_error)?;
                    }
                    Ok(())
                }
                Err(error) => Err(internal_error(error)),
            },
            Err(error) => {
                let error_text = error.to_string();
                state
                    .db
                    .record_error(id, &error_text)
                    .await
                    .map_err(internal_error)?;
                Ok(())
            }
        },
        Err(error) => Err(internal_error(error)),
    }
}

fn authorize_optional(state: &HttpState, headers: &HeaderMap) -> Option<axum::response::Response> {
    let Some(token) = &state.config.api_token else {
        return None;
    };
    authorize_token(token, headers)
}

fn authorize_required(state: &HttpState, headers: &HeaderMap) -> Option<axum::response::Response> {
    let Some(token) = &state.config.api_token else {
        return Some(
            (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    error: "WEBWATCH_API_TOKEN is required for this endpoint".to_string(),
                }),
            )
                .into_response(),
        );
    };
    authorize_token(token, headers)
}

fn authorize_token(token: &str, headers: &HeaderMap) -> Option<axum::response::Response> {
    let expected = format!("Bearer {token}");
    let authorized = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .map(|value| value == expected)
        .unwrap_or(false);
    if authorized {
        return None;
    }

    Some(
        (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "missing or invalid bearer token".to_string(),
            }),
        )
            .into_response(),
    )
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
