#![allow(dead_code)]

//! Data Transfer Objects for the REST API layer.
//!
//! DTOs are the shapes of JSON request and response bodies.
//! They are distinct from domain types and must be mapped explicitly
//! — no domain type is ever serialised directly into an HTTP response.

use serde::{Deserialize, Serialize};

// ── Requests ──────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateRequest {
    pub payload: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct QueryRequest {
    #[serde(default)]
    pub filter: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct ValidateRequest {
    pub payload: serde_json::Value,
}

// ── Responses ─────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct RecordResponse {
    pub id: String,
    pub model: String,
    pub version: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct ValidationResponse {
    pub model: String,
    pub version: String,
    pub passed: bool,
    pub results: Vec<ValidatorResultDto>,
}

#[derive(Debug, Serialize)]
pub struct ValidatorResultDto {
    pub kind: String,
    pub passed: bool,
    pub violations: Vec<ViolationDto>,
}

#[derive(Debug, Serialize)]
pub struct ViolationDto {
    pub path: Option<String>,
    pub message: String,
    pub severity: String,
}

#[derive(Debug, Serialize)]
pub struct ModelListResponse {
    pub models: Vec<ModelDescriptorDto>,
}

#[derive(Debug, Serialize)]
pub struct ModelDescriptorDto {
    pub id: String,
    pub version: String,
}

#[derive(Debug, Serialize)]
pub struct RefreshResponse {
    pub refreshed_at: String,
    pub models_found: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
}
