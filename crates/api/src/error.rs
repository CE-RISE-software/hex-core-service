use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use hex_core::domain::error::CoreError;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum ApiError {
    Core(CoreError),
    MissingIdempotencyKey,
    Unauthorized(String),
    BadRequest(String),
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, body) = match self {
            ApiError::MissingIdempotencyKey => (
                StatusCode::BAD_REQUEST,
                ErrorBody {
                    code: "MISSING_IDEMPOTENCY_KEY".into(),
                    message: "Idempotency-Key header is required for this operation".into(),
                    details: None,
                },
            ),

            ApiError::Unauthorized(msg) => (
                StatusCode::UNAUTHORIZED,
                ErrorBody {
                    code: "UNAUTHORIZED".into(),
                    message: msg,
                    details: None,
                },
            ),

            ApiError::BadRequest(msg) => (
                StatusCode::BAD_REQUEST,
                ErrorBody {
                    code: "BAD_REQUEST".into(),
                    message: msg,
                    details: None,
                },
            ),

            ApiError::Internal(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                ErrorBody {
                    code: "INTERNAL_ERROR".into(),
                    message: msg,
                    details: None,
                },
            ),

            ApiError::Core(err) => match err {
                CoreError::ModelNotFound { model, version } => (
                    StatusCode::NOT_FOUND,
                    ErrorBody {
                        code: "MODEL_NOT_FOUND".into(),
                        message: format!("Model {model} v{version} not found in registry"),
                        details: None,
                    },
                ),

                CoreError::ValidationFailed(report) => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    ErrorBody {
                        code: "VALIDATION_FAILED".into(),
                        message: "Payload failed one or more validators".into(),
                        details: serde_json::to_value(&report).ok(),
                    },
                ),

                CoreError::IdempotencyConflict { key } => (
                    StatusCode::CONFLICT,
                    ErrorBody {
                        code: "IDEMPOTENCY_CONFLICT".into(),
                        message: format!(
                            "Idempotency-Key '{key}' was already used with a different payload"
                        ),
                        details: None,
                    },
                ),

                CoreError::Store(e) => (
                    StatusCode::BAD_GATEWAY,
                    ErrorBody {
                        code: "STORE_ERROR".into(),
                        message: e.to_string(),
                        details: None,
                    },
                ),

                CoreError::Registry(e) => (
                    StatusCode::BAD_GATEWAY,
                    ErrorBody {
                        code: "REGISTRY_ERROR".into(),
                        message: e.to_string(),
                        details: None,
                    },
                ),

                CoreError::Validator(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ErrorBody {
                        code: "VALIDATOR_ERROR".into(),
                        message: e.to_string(),
                        details: None,
                    },
                ),

                CoreError::Enricher(e) => (
                    StatusCode::BAD_GATEWAY,
                    ErrorBody {
                        code: "ENRICHER_ERROR".into(),
                        message: e.to_string(),
                        details: None,
                    },
                ),

                CoreError::Internal(msg) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ErrorBody {
                        code: "INTERNAL_ERROR".into(),
                        message: msg,
                        details: None,
                    },
                ),

                _ => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ErrorBody {
                        code: "INTERNAL_ERROR".into(),
                        message: "An unexpected error occurred".into(),
                        details: None,
                    },
                ),
            },
        };

        (status, Json(body)).into_response()
    }
}

impl From<CoreError> for ApiError {
    fn from(err: CoreError) -> Self {
        ApiError::Core(err)
    }
}
