use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use hex_core::domain::model::{ModelId, ModelVersion};
use std::sync::Arc;

use crate::AppState;

/// GET /models
pub async fn list(State(state): State<Arc<AppState>>) -> Response {
    match state.registry.list_models().await {
        Ok(models) => {
            let items: Vec<serde_json::Value> = models
                .into_iter()
                .map(|m| serde_json::json!({ "id": m.id.0, "version": m.version.0 }))
                .collect();
            Json(serde_json::json!({ "models": items })).into_response()
        }
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({ "code": "REGISTRY_ERROR", "message": e.to_string() })),
        )
            .into_response(),
    }
}

/// GET /openapi.json
pub async fn openapi_spec() -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json; charset=utf-8")
        .body(axum::body::Body::from(include_str!("../openapi.json")))
        .unwrap()
}

/// GET /models/{model}/versions/{version}/schema
pub async fn artifact_schema(
    State(state): State<Arc<AppState>>,
    Path((model, version)): Path<(String, String)>,
) -> Response {
    text_artifact(&state, &model, &version, |a| a.schema.clone()).await
}

/// GET /models/{model}/versions/{version}/shacl
pub async fn artifact_shacl(
    State(state): State<Arc<AppState>>,
    Path((model, version)): Path<(String, String)>,
) -> Response {
    text_artifact(&state, &model, &version, |a| a.shacl.clone()).await
}

/// GET /models/{model}/versions/{version}/owl
pub async fn artifact_owl(
    State(state): State<Arc<AppState>>,
    Path((model, version)): Path<(String, String)>,
) -> Response {
    text_artifact(&state, &model, &version, |a| a.owl.clone()).await
}

/// GET /models/{model}/versions/{version}/route
pub async fn artifact_route(
    State(state): State<Arc<AppState>>,
    Path((model, version)): Path<(String, String)>,
) -> Response {
    let model_id = ModelId(model);
    let model_version = ModelVersion(version);

    match state.registry.resolve(&model_id, &model_version).await {
        Ok(artifacts) => match artifacts.route {
            Some(route) => Json(route).into_response(),
            None => not_found("route artifact not present for this model version"),
        },
        Err(e) => registry_error(e),
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

async fn text_artifact<F>(
    state: &Arc<AppState>,
    model: &str,
    version: &str,
    extractor: F,
) -> Response
where
    F: Fn(&hex_core::domain::model::ArtifactSet) -> Option<String>,
{
    let model_id = ModelId(model.to_string());
    let model_version = ModelVersion(version.to_string());

    match state.registry.resolve(&model_id, &model_version).await {
        Ok(artifacts) => match extractor(&artifacts) {
            Some(text) => Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "text/plain; charset=utf-8")
                .body(axum::body::Body::from(text))
                .unwrap(),
            None => not_found("artifact not present for this model version"),
        },
        Err(e) => registry_error(e),
    }
}

fn not_found(message: &str) -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({ "code": "NOT_FOUND", "message": message })),
    )
        .into_response()
}

fn registry_error(e: hex_core::domain::error::RegistryError) -> Response {
    (
        StatusCode::BAD_GATEWAY,
        Json(serde_json::json!({ "code": "REGISTRY_ERROR", "message": e.to_string() })),
    )
        .into_response()
}
