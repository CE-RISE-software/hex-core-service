use std::sync::Arc;

use crate::domain::{
    auth::SecurityContext,
    error::CoreError,
    model::{ModelId, ModelVersion},
    validation::ValidationReport,
};
use crate::ports::{
    inbound::validate::ValidateUseCase,
    outbound::{registry::ArtifactRegistryPort, validator::ValidatorPort},
};

pub struct ValidateUseCaseImpl {
    registry: Arc<dyn ArtifactRegistryPort>,
    validators: Vec<Arc<dyn ValidatorPort>>,
}

impl ValidateUseCaseImpl {
    pub fn new(
        registry: Arc<dyn ArtifactRegistryPort>,
        validators: Vec<Arc<dyn ValidatorPort>>,
    ) -> Self {
        Self {
            registry,
            validators,
        }
    }
}

#[async_trait::async_trait]
impl ValidateUseCase for ValidateUseCaseImpl {
    async fn validate(
        &self,
        _ctx: &SecurityContext,
        model: &ModelId,
        version: &ModelVersion,
        payload: &serde_json::Value,
    ) -> Result<ValidationReport, CoreError> {
        // 1. Resolve artifact set
        let artifacts = self.registry.resolve(model, version).await?;

        // 2. Assert routable
        if !artifacts.is_routable() {
            return Err(CoreError::NotRoutable);
        }

        // 3. Run each validator; skip if its required artifact is absent
        let mut results = Vec::new();
        for validator in &self.validators {
            let result = validator
                .validate(&artifacts, payload)
                .await
                .map_err(CoreError::Validator)?;
            results.push(result);
        }

        // 4. Merge into a single report
        Ok(ValidationReport::new(
            model.clone(),
            version.clone(),
            results,
        ))
    }
}
