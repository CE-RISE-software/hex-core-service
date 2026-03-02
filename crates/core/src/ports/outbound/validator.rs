use crate::domain::{
    error::ValidatorError,
    model::ArtifactSet,
    validation::{ValidationResult, ValidatorKind},
};

#[async_trait::async_trait]
pub trait ValidatorPort: Send + Sync {
    fn kind(&self) -> ValidatorKind;

    /// Validate `payload` against the artifacts in `artifact_set`.
    /// Returns a `ValidationResult` regardless of pass/fail.
    /// Returns `Err` only on execution failure (not on validation violations).
    async fn validate(
        &self,
        artifacts: &ArtifactSet,
        payload: &serde_json::Value,
    ) -> Result<ValidationResult, ValidatorError>;
}
