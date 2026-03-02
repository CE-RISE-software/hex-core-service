//! SHACL validator — stub implementation of `ValidatorPort`.
//!
//! SHACL is the preferred validation path (see AGENTS.md §8).
//! A mature Rust-native SHACL engine is not yet available; this stub
//! documents the intended interface and can be backed by an external
//! process or library once one is selected.

use async_trait::async_trait;
use hex_core::domain::{
    error::ValidatorError,
    model::ArtifactSet,
    validation::{ValidationResult, ValidatorKind},
};
use hex_core::ports::outbound::validator::ValidatorPort;

pub struct ShaclValidator;

impl ShaclValidator {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ShaclValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ValidatorPort for ShaclValidator {
    fn kind(&self) -> ValidatorKind {
        ValidatorKind::Shacl
    }

    async fn validate(
        &self,
        artifacts: &ArtifactSet,
        _payload: &serde_json::Value,
    ) -> Result<ValidationResult, ValidatorError> {
        let shacl = match &artifacts.shacl {
            Some(s) => s,
            None => {
                // No SHACL artifact present — skip gracefully.
                return Ok(ValidationResult {
                    kind: ValidatorKind::Shacl,
                    passed: true,
                    violations: vec![],
                });
            }
        };

        // TODO: integrate a SHACL engine (e.g. via subprocess or FFI).
        // Until then, log the shapes graph size and return a not-implemented error.
        let _ = shacl;
        Err(ValidatorError::Execution(
            "SHACL engine not yet implemented".into(),
        ))
    }
}
