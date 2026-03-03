use std::sync::Arc;

use crate::domain::{
    auth::SecurityContext,
    error::CoreError,
    model::{ModelId, ModelVersion},
    record::Record,
};
use crate::ports::{
    inbound::record::RecordUseCase,
    outbound::{
        record_store::RecordStorePort, registry::ArtifactRegistryPort, validator::ValidatorPort,
    },
};

pub struct RecordUseCaseImpl {
    pub registry: Arc<dyn ArtifactRegistryPort>,
    pub validators: Vec<Arc<dyn ValidatorPort>>,
    pub store: Arc<dyn RecordStorePort>,
}

#[async_trait::async_trait]
impl RecordUseCase for RecordUseCaseImpl {
    async fn create(
        &self,
        ctx: &SecurityContext,
        idempotency_key: &str,
        model: &ModelId,
        version: &ModelVersion,
        payload: serde_json::Value,
    ) -> Result<Record, CoreError> {
        let artifacts = self.registry.resolve(model, version).await?;

        if !artifacts.is_routable() {
            return Err(CoreError::NotRoutable);
        }

        // Run all validators; collect results.
        let mut results = Vec::new();
        for v in &self.validators {
            if let Ok(result) = v.validate(&artifacts, &payload).await {
                results.push(result);
            }
        }

        let report = crate::domain::validation::ValidationReport::new(
            model.clone(),
            version.clone(),
            results,
        );

        if !report.passed {
            return Err(CoreError::ValidationFailed(report));
        }

        let id = crate::domain::record::RecordId(uuid());
        let record = Record {
            id: id.clone(),
            model: model.clone(),
            version: version.clone(),
            payload,
        };

        self.store
            .write(ctx, idempotency_key, record.clone())
            .await?;

        Ok(record)
    }

    async fn query(
        &self,
        ctx: &SecurityContext,
        model: &ModelId,
        version: &ModelVersion,
        filter: serde_json::Value,
    ) -> Result<Vec<Record>, CoreError> {
        let artifacts = self.registry.resolve(model, version).await?;

        if !artifacts.is_routable() {
            return Err(CoreError::NotRoutable);
        }

        Ok(self.store.query(ctx, filter).await?)
    }
}

fn uuid() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    // Minimal ID generation for scaffolding — replace with `uuid` crate in production.
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    format!("record-{nanos:08x}")
}
