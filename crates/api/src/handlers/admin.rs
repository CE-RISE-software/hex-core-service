use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde_json::json;
use std::sync::Arc;
use std::time::Instant;

use crate::AppState;

pub async fn health(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let uptime = Instant::now().duration_since(state.started_at).as_secs();
    (
        StatusCode::OK,
        Json(json!({ "status": "ok", "uptime_seconds": uptime })),
    )
}

pub async fn ready(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.registry.list_models().await {
        Ok(models) if !models.is_empty() => (
            StatusCode::OK,
            Json(json!({
                "status": "ready",
                "registry_loaded": true,
                "models_available": models.len()
            })),
        ),
        Ok(models) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "status": "not_ready",
                "registry_loaded": true,
                "models_available": models.len()
            })),
        ),
        Err(e) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "status": "not_ready",
                "registry_loaded": false,
                "reason": e.to_string()
            })),
        ),
    }
}

pub async fn status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let models_indexed = state
        .registry
        .list_models()
        .await
        .map(|m| m.len())
        .unwrap_or(0);
    let uptime = Instant::now().duration_since(state.started_at).as_secs();

    (
        StatusCode::OK,
        Json(json!({
            "status": "ok",
            "uptime_seconds": uptime,
            "registry": {
                "models_loaded": models_indexed
            },
            "config": {
                "metrics_enabled": state.metrics_enabled
            }
        })),
    )
}

pub async fn metrics(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    if !state.metrics_enabled {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "metrics disabled (set METRICS_ENABLED=true)" })),
        )
            .into_response();
    }

    (
        StatusCode::OK,
        [("content-type", "text/plain; version=0.0.4; charset=utf-8")],
        state.metrics.render_prometheus(),
    )
        .into_response()
}

pub async fn registry_refresh(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.registry.refresh().await {
        Ok(summary) => (StatusCode::OK, Json(json!(summary))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

pub async fn config() -> impl IntoResponse {
    // TODO: return a redacted config dump.
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({ "error": "config endpoint not yet implemented" })),
    )
}

pub async fn cache_clear() -> impl IntoResponse {
    // TODO: clear artifact cache when caching is enabled.
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({ "error": "cache clear not yet implemented" })),
    )
}
