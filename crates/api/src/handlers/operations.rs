use axum::{
    extract::{Path, State},
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
pub struct ModelOperationPath {
    pub model: String,
    pub version: String,
    pub operation: String,
}

/// Unified operation entrypoint:
/// `POST /models/{model}/versions/{version}:{operation}`
pub async fn dispatch(
    State(state): State<Arc<AppState>>,
    Path(path): Path<ModelOperationPath>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Result<Response, ApiError> {
    let model_path = ModelPath {
        model: path.model,
        version: path.version,
    };

    match path.operation.as_str() {
        "validate" => {
            let request: ValidateRequest = serde_json::from_value(body)
                .map_err(|e| ApiError::BadRequest(format!("invalid validate body: {e}")))?;
            validate(State(state), Path(model_path), headers, Json(request))
                .await
                .map(IntoResponse::into_response)
        }
        "create" => {
            let request: CreateRequest = serde_json::from_value(body)
                .map_err(|e| ApiError::BadRequest(format!("invalid create body: {e}")))?;
            create(State(state), Path(model_path), headers, Json(request))
                .await
                .map(IntoResponse::into_response)
        }
        "query" => {
            let request: QueryRequest = serde_json::from_value(body)
                .map_err(|e| ApiError::BadRequest(format!("invalid query body: {e}")))?;
            query(State(state), Path(model_path), headers, Json(request))
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
    headers: HeaderMap,
    Json(body): Json<ValidateRequest>,
) -> Result<Json<ValidateResponse>, ApiError> {
    let ctx = extract_security_context(&headers)?;
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
    headers: HeaderMap,
    Json(body): Json<CreateRequest>,
) -> Result<Json<CreateResponse>, ApiError> {
    let ctx = extract_security_context(&headers)?;
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
    headers: HeaderMap,
    Json(body): Json<QueryRequest>,
) -> Result<Json<QueryResponse>, ApiError> {
    let ctx = extract_security_context(&headers)?;
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

fn extract_security_context(headers: &HeaderMap) -> Result<SecurityContext, ApiError> {
    // TODO: replace with real JWT validation middleware (see crate::auth).
    // For now, extract a minimal stub context so the handler compiles and routes.
    let subject = headers
        .get("x-subject")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("anonymous")
        .to_string();

    let raw_token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|t| t.to_string());

    Ok(SecurityContext {
        subject,
        roles: vec![],
        scopes: vec![],
        tenant: None,
        raw_token,
    })
}

fn extract_idempotency_key(headers: &HeaderMap) -> Result<String, ApiError> {
    headers
        .get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .ok_or(ApiError::MissingIdempotencyKey)
}
