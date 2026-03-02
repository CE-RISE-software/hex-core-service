use async_trait::async_trait;

use crate::domain::{
    auth::SecurityContext,
    error::CoreError,
    record::{Record, RecordId},
};

#[async_trait]
pub trait EnrichUseCase: Send + Sync {
    async fn enrich(
        &self,
        ctx: &SecurityContext,
        idempotency_key: &str,
        record_id: &RecordId,
    ) -> Result<Record, CoreError>;
}
