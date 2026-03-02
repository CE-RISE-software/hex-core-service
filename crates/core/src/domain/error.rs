use crate::domain::validation::ValidationReport;
use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum StoreError {
    #[error("record not found: {id}")]
    NotFound { id: String },
    #[error("idempotency conflict: key {key} already used with a different payload")]
    IdempotencyConflict { key: String },
    #[error("store unavailable: {0}")]
    Unavailable(String),
    #[error("store internal error: {0}")]
    Internal(String),
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum RegistryError {
    #[error("model not found in registry: {model} v{version}")]
    ModelNotFound { model: String, version: String },
    #[error("artifact fetch failed for {url}: {reason}")]
    FetchFailed { url: String, reason: String },
    #[error("disallowed registry host: {host}")]
    DisallowedHost { host: String },
    #[error("registry requires HTTPS but got: {url}")]
    InsecureUrl { url: String },
    #[error("registry internal error: {0}")]
    Internal(String),
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ValidatorError {
    #[error("validator initialisation failed: {0}")]
    Init(String),
    #[error("validator execution failed: {0}")]
    Execution(String),
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EnricherError {
    #[error("enricher unavailable: {0}")]
    Unavailable(String),
    #[error("enricher internal error: {0}")]
    Internal(String),
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CoreError {
    #[error("model not found: {model} v{version}")]
    ModelNotFound { model: String, version: String },

    #[error("model artifact is not routable: missing route definition")]
    NotRoutable,

    #[error("validation failed")]
    ValidationFailed(ValidationReport),

    #[error("idempotency conflict: key {key} already used with a different payload")]
    IdempotencyConflict { key: String },

    #[error("store error: {0}")]
    Store(#[from] StoreError),

    #[error("registry error: {0}")]
    Registry(#[from] RegistryError),

    #[error("validator error: {0}")]
    Validator(#[from] ValidatorError),

    #[error("enricher error: {0}")]
    Enricher(#[from] EnricherError),

    #[error("internal error: {0}")]
    Internal(String),
}
