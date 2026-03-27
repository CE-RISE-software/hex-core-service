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

#[cfg(test)]
mod tests {
    use super::{CoreError, EnricherError, RegistryError, StoreError, ValidatorError};
    use crate::domain::model::{ModelId, ModelVersion};
    use crate::domain::validation::{ValidationReport, ValidationResult, ValidatorKind};

    #[test]
    fn store_error_variants_render_messages() {
        assert_eq!(
            StoreError::NotFound { id: "rec-1".into() }.to_string(),
            "record not found: rec-1"
        );
        assert_eq!(
            StoreError::IdempotencyConflict { key: "abc".into() }.to_string(),
            "idempotency conflict: key abc already used with a different payload"
        );
        assert_eq!(
            StoreError::Unavailable("down".into()).to_string(),
            "store unavailable: down"
        );
        assert_eq!(
            StoreError::Internal("boom".into()).to_string(),
            "store internal error: boom"
        );
    }

    #[test]
    fn registry_error_variants_render_messages() {
        assert_eq!(
            RegistryError::ModelNotFound {
                model: "dp".into(),
                version: "1.0.0".into()
            }
            .to_string(),
            "model not found in registry: dp v1.0.0"
        );
        assert_eq!(
            RegistryError::FetchFailed {
                url: "https://example.test/schema.json".into(),
                reason: "HTTP 500".into()
            }
            .to_string(),
            "artifact fetch failed for https://example.test/schema.json: HTTP 500"
        );
        assert_eq!(
            RegistryError::DisallowedHost {
                host: "bad.test".into()
            }
            .to_string(),
            "disallowed registry host: bad.test"
        );
        assert_eq!(
            RegistryError::InsecureUrl {
                url: "http://bad.test".into()
            }
            .to_string(),
            "registry requires HTTPS but got: http://bad.test"
        );
        assert_eq!(
            RegistryError::Internal("oops".into()).to_string(),
            "registry internal error: oops"
        );
    }

    #[test]
    fn validator_and_enricher_errors_render_messages() {
        assert_eq!(
            ValidatorError::Init("missing engine".into()).to_string(),
            "validator initialisation failed: missing engine"
        );
        assert_eq!(
            ValidatorError::Execution("runtime fault".into()).to_string(),
            "validator execution failed: runtime fault"
        );
        assert_eq!(
            EnricherError::Unavailable("offline".into()).to_string(),
            "enricher unavailable: offline"
        );
        assert_eq!(
            EnricherError::Internal("panic".into()).to_string(),
            "enricher internal error: panic"
        );
    }

    #[test]
    fn core_error_variants_render_messages() {
        assert_eq!(
            CoreError::ModelNotFound {
                model: "dp".into(),
                version: "1.0.0".into()
            }
            .to_string(),
            "model not found: dp v1.0.0"
        );
        assert_eq!(
            CoreError::IdempotencyConflict { key: "abc".into() }.to_string(),
            "idempotency conflict: key abc already used with a different payload"
        );
        assert_eq!(
            CoreError::Internal("unexpected".into()).to_string(),
            "internal error: unexpected"
        );
    }

    #[test]
    fn core_error_wraps_sub_errors_via_from() {
        let store = CoreError::from(StoreError::Unavailable("down".into()));
        assert!(matches!(store, CoreError::Store(_)));

        let registry = CoreError::from(RegistryError::Internal("boom".into()));
        assert!(matches!(registry, CoreError::Registry(_)));

        let validator = CoreError::from(ValidatorError::Execution("bad".into()));
        assert!(matches!(validator, CoreError::Validator(_)));

        let enricher = CoreError::from(EnricherError::Unavailable("nope".into()));
        assert!(matches!(enricher, CoreError::Enricher(_)));
    }

    #[test]
    fn core_error_validation_failed_variant_holds_report() {
        let report = ValidationReport::new(
            ModelId("product-passport".into()),
            ModelVersion("1.0.0".into()),
            vec![ValidationResult {
                kind: ValidatorKind::JsonSchema,
                passed: false,
                violations: vec![],
            }],
        );

        let err = CoreError::ValidationFailed(report.clone());
        match err {
            CoreError::ValidationFailed(r) => {
                assert_eq!(r.model.0, report.model.0);
                assert_eq!(r.version.0, report.version.0);
                assert!(!r.passed);
            }
            _ => panic!("expected ValidationFailed variant"),
        }
    }
}
