use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde_json::json;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Instant;

use crate::AppState;

static OPENAPI_VERSION: OnceLock<String> = OnceLock::new();

fn openapi_spec_version() -> &'static str {
    OPENAPI_VERSION
        .get_or_init(|| {
            serde_json::from_str::<serde_json::Value>(include_str!("../openapi.json"))
                .ok()
                .and_then(|v| {
                    v.get("info")
                        .and_then(|i| i.get("version"))
                        .and_then(|s| s.as_str())
                        .map(str::to_string)
                })
                .unwrap_or_else(|| "unknown".to_string())
        })
        .as_str()
}

pub async fn health(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let uptime = Instant::now().duration_since(state.started_at).as_secs();
    (
        StatusCode::OK,
        Json(json!({ "status": "ok", "uptime_seconds": uptime })),
    )
}

pub async fn version() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({
            "service": "hex-core-service",
            "service_version": env!("CARGO_PKG_VERSION"),
            "openapi_version": openapi_spec_version()
        })),
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

#[cfg(test)]
mod tests {
    use super::{openapi_spec_version, version};
    use axum::body::to_bytes;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use serde_json::Value;

    #[test]
    fn openapi_version_is_available() {
        assert!(
            !openapi_spec_version().is_empty(),
            "openapi version should be discoverable from openapi.json"
        );
    }

    #[tokio::test]
    async fn version_endpoint_returns_service_and_openapi_versions() {
        let response = version().await.into_response();
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read response body");
        let json: Value = serde_json::from_slice(&body).expect("parse version response JSON");

        assert_eq!(json["service"], "hex-core-service");
        assert_eq!(json["service_version"], env!("CARGO_PKG_VERSION"));
        assert!(
            json["openapi_version"].as_str().is_some(),
            "openapi_version should be present as string"
        );
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
