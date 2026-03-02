use crate::domain::auth::SecurityContext;
use crate::domain::error::CoreError;
use crate::domain::model::{ModelId, ModelVersion};
use crate::domain::record::Record;

#[async_trait::async_trait]
pub trait RecordUseCase: Send + Sync {
    async fn create(
        &self,
        ctx: &SecurityContext,
        idempotency_key: &str,
        model: &ModelId,
        version: &ModelVersion,
        payload: serde_json::Value,
    ) -> Result<Record, CoreError>;

    async fn query(
        &self,
        ctx: &SecurityContext,
        model: &ModelId,
        version: &ModelVersion,
        filter: serde_json::Value,
    ) -> Result<Vec<Record>, CoreError>;
}
