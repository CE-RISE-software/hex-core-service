use std::sync::Arc;

use axum::{
    routing::{get, post},
    Router,
};

use crate::handlers::{admin, models, operations};
use crate::AppState;

pub fn build(state: Arc<AppState>) -> Router {
    Router::new()
        // ── Model operations ──────────────────────────────────────────────────
        .route(
            "/models/{model}/versions/{version}:validate",
            post(operations::validate),
        )
        .route(
            "/models/{model}/versions/{version}:create",
            post(operations::create),
        )
        .route(
            "/models/{model}/versions/{version}:query",
            post(operations::query),
        )
        // ── Public introspection ──────────────────────────────────────────────
        .route("/models", get(models::list))
        .route(
            "/models/{model}/versions/{version}/schema",
            get(models::artifact_schema),
        )
        .route(
            "/models/{model}/versions/{version}/shacl",
            get(models::artifact_shacl),
        )
        .route(
            "/models/{model}/versions/{version}/owl",
            get(models::artifact_owl),
        )
        .route(
            "/models/{model}/versions/{version}/route",
            get(models::artifact_route),
        )
        // ── OpenAPI self-description ──────────────────────────────────────────
        .route("/openapi.json", get(models::openapi_spec))
        // ── Admin ─────────────────────────────────────────────────────────────
        .route("/admin/health", get(admin::health))
        .route("/admin/ready", get(admin::ready))
        .route("/admin/status", get(admin::status))
        .route("/admin/metrics", get(admin::metrics))
        .route("/admin/registry/refresh", post(admin::registry_refresh))
        .route("/admin/config", get(admin::config))
        .route("/admin/cache/clear", post(admin::cache_clear))
        .with_state(state)
}
