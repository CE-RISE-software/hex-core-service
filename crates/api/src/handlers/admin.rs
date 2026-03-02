use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde_json::json;
use std::sync::Arc;

use hex_core::ports::outbound::registry::ArtifactRegistryPort;

use crate::AppState;

pub async fn health() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({ "status": "ok" })))
}

pub async fn ready(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.registry.list_models().await {
        Ok(_) => (StatusCode::OK, Json(json!({ "status": "ready" }))),
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "status": "not_ready" })),
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

    (
        StatusCode::OK,
        Json(json!({
            "status": "ok",
            "models_indexed": models_indexed,
        })),
    )
}

pub async fn metrics() -> impl IntoResponse {
    // TODO: expose Prometheus metrics when METRICS_ENABLED=true.
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({ "error": "metrics not yet implemented" })),
    )
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
