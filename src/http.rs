use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Serialize;
use tower_http::trace::TraceLayer;

use crate::{
    config::{AppConfig, Target, TargetStatus},
    db,
    db::Persistence,
    discord, evaluator,
};

#[derive(Clone)]
pub struct HttpState {
    pub config: Arc<AppConfig>,
    pub targets: Arc<Vec<Target>>,
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

pub fn router(state: HttpState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/targets", get(targets))
        .route("/targets/:id/status", get(target_status))
        .route("/notify/status", post(notify_status))
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

    for target in state.targets.iter().filter(|target| target.enabled()) {
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

async fn check_target_by_id(
    state: &HttpState,
    id: &str,
    mark_manual_report: bool,
) -> Result<(), axum::response::Response> {
    let Some(target_config) = state.targets.iter().find(|target| target.id == id) else {
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
