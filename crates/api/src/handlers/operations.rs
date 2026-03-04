use axum::{
    extract::{Extension, Path, State},
    http::HeaderMap,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use hex_core::domain::{
    auth::SecurityContext,
    model::{ModelId, ModelVersion},
};

use crate::{error::ApiError, AppState};

#[derive(Debug, Deserialize)]
pub struct ModelPath {
    pub model: String,
    pub version: String,
}

#[derive(Debug, Deserialize)]
pub struct RawOperationPath {
    pub model: String,
    pub version_op: String,
}

#[derive(Debug, Deserialize)]
pub struct SlashOperationPath {
    pub model: String,
    pub version: String,
    pub operation: String,
}

/// Unified operation entrypoint:
/// `POST /models/{model}/versions/{version}:{operation}`
pub async fn dispatch(
    State(state): State<Arc<AppState>>,
    Path(path): Path<RawOperationPath>,
    ctx: Option<Extension<SecurityContext>>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Result<Response, ApiError> {
    let (version, operation) = path.version_op.split_once(':').ok_or_else(|| {
        ApiError::BadRequest(
            "invalid version/operation segment; expected '{version}:{operation}'".into(),
        )
    })?;

    dispatch_with_parts(
        state,
        path.model,
        version.to_string(),
        operation.to_string(),
        ctx.map(|Extension(c)| c),
        headers,
        body,
    )
    .await
}

/// Compatibility operation entrypoint:
/// `POST /models/{model}/versions/{version}/{operation}`
pub async fn dispatch_slash(
    State(state): State<Arc<AppState>>,
    Path(path): Path<SlashOperationPath>,
    ctx: Option<Extension<SecurityContext>>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Result<Response, ApiError> {
    dispatch_with_parts(
        state,
        path.model,
        path.version,
        path.operation,
        ctx.map(|Extension(c)| c),
        headers,
        body,
    )
    .await
}

async fn dispatch_with_parts(
    state: Arc<AppState>,
    model: String,
    version: String,
    operation: String,
    ctx: Option<SecurityContext>,
    headers: HeaderMap,
    body: serde_json::Value,
) -> Result<Response, ApiError> {
    let model_path = ModelPath { model, version };
    let extension_ctx = ctx.map(Extension);

    match operation.as_str() {
        "validate" => {
            let request: ValidateRequest = serde_json::from_value(body)
                .map_err(|e| ApiError::BadRequest(format!("invalid validate body: {e}")))?;
            validate(
                State(state),
                Path(model_path),
                extension_ctx.clone(),
                headers,
                Json(request),
            )
            .await
            .map(IntoResponse::into_response)
        }
        "create" => {
            let request: CreateRequest = serde_json::from_value(body)
                .map_err(|e| ApiError::BadRequest(format!("invalid create body: {e}")))?;
            create(
                State(state),
                Path(model_path),
                extension_ctx.clone(),
                headers,
                Json(request),
            )
            .await
            .map(IntoResponse::into_response)
        }
        "query" => {
            let request: QueryRequest = serde_json::from_value(body)
                .map_err(|e| ApiError::BadRequest(format!("invalid query body: {e}")))?;
            query(
                State(state),
                Path(model_path),
                extension_ctx,
                headers,
                Json(request),
            )
            .await
            .map(IntoResponse::into_response)
        }
        other => Err(ApiError::BadRequest(format!(
            "unsupported operation '{other}', expected one of: validate, create, query"
        ))),
    }
}

impl ModelPath {
    pub fn model_id(&self) -> ModelId {
        ModelId(self.model.clone())
    }
    pub fn model_version(&self) -> ModelVersion {
        ModelVersion(self.version.clone())
    }
}

// ─── :validate ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ValidateRequest {
    pub payload: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct ValidateResponse {
    pub passed: bool,
    pub results: serde_json::Value,
}

pub async fn validate(
    State(state): State<Arc<AppState>>,
    Path(path): Path<ModelPath>,
    ctx: Option<Extension<SecurityContext>>,
    headers: HeaderMap,
    Json(body): Json<ValidateRequest>,
) -> Result<Json<ValidateResponse>, ApiError> {
    let ctx = require_security_context(ctx)?;
    let _ = headers;
    let model = path.model_id();
    let version = path.model_version();

    let report = state
        .validate_use_case
        .validate(&ctx, &model, &version, &body.payload)
        .await?;

    Ok(Json(ValidateResponse {
        passed: report.passed,
        results: serde_json::to_value(&report.results).unwrap_or_default(),
    }))
}

// ─── :create ─────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateRequest {
    pub payload: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct CreateResponse {
    pub id: String,
    pub model: String,
    pub version: String,
    pub payload: serde_json::Value,
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    Path(path): Path<ModelPath>,
    ctx: Option<Extension<SecurityContext>>,
    headers: HeaderMap,
    Json(body): Json<CreateRequest>,
) -> Result<Json<CreateResponse>, ApiError> {
    let ctx = require_security_context(ctx)?;
    let idempotency_key = extract_idempotency_key(&headers)?;
    let model = path.model_id();
    let version = path.model_version();

    let record = state
        .record_use_case
        .create(&ctx, &idempotency_key, &model, &version, body.payload)
        .await?;

    Ok(Json(CreateResponse {
        id: record.id.0,
        model: record.model.0,
        version: record.version.0,
        payload: record.payload,
    }))
}

// ─── :query ──────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct QueryRequest {
    #[serde(default)]
    pub filter: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct QueryResponse {
    pub records: Vec<serde_json::Value>,
}

pub async fn query(
    State(state): State<Arc<AppState>>,
    Path(path): Path<ModelPath>,
    ctx: Option<Extension<SecurityContext>>,
    headers: HeaderMap,
    Json(body): Json<QueryRequest>,
) -> Result<Json<QueryResponse>, ApiError> {
    let ctx = require_security_context(ctx)?;
    let _ = headers;
    let model = path.model_id();
    let version = path.model_version();

    let records = state
        .record_use_case
        .query(&ctx, &model, &version, body.filter)
        .await?;

    let records = records
        .into_iter()
        .map(|r| serde_json::to_value(r).unwrap_or_default())
        .collect();

    Ok(Json(QueryResponse { records }))
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn require_security_context(
    ctx: Option<Extension<SecurityContext>>,
) -> Result<SecurityContext, ApiError> {
    ctx.map(|Extension(c)| c)
        .ok_or_else(|| ApiError::Unauthorized("missing validated security context".into()))
}

fn extract_idempotency_key(headers: &HeaderMap) -> Result<String, ApiError> {
    headers
        .get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .ok_or(ApiError::MissingIdempotencyKey)
}
