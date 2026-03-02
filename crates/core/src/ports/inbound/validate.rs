use async_trait::async_trait;

use crate::domain::{
    auth::SecurityContext,
    error::CoreError,
    model::{ModelId, ModelVersion},
    validation::ValidationReport,
};

#[async_trait]
pub trait ValidateUseCase: Send + Sync {
    async fn validate(
        &self,
        ctx: &SecurityContext,
        model: &ModelId,
        version: &ModelVersion,
        payload: &serde_json::Value,
    ) -> Result<ValidationReport, CoreError>;
}
