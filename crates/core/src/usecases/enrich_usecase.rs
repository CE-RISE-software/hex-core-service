use std::sync::Arc;

use async_trait::async_trait;

use crate::domain::{
    auth::SecurityContext,
    error::CoreError,
    record::{Record, RecordId},
};
use crate::ports::{
    inbound::enrich::EnrichUseCase,
    outbound::{enricher::EnricherPort, record_store::RecordStorePort},
};

pub struct EnrichUseCaseImpl {
    pub store: Arc<dyn RecordStorePort>,
    pub enricher: Arc<dyn EnricherPort>,
}

#[async_trait]
impl EnrichUseCase for EnrichUseCaseImpl {
    async fn enrich(
        &self,
        ctx: &SecurityContext,
        idempotency_key: &str,
        record_id: &RecordId,
    ) -> Result<Record, CoreError> {
        // 1. Read existing record
        let record = self.store.read(ctx, record_id).await?;

        // 2. Compute enrichment
        let enriched_payload = self.enricher.enrich(ctx, &record).await?;

        // 3. Write enriched record back
        let enriched = Record {
            payload: enriched_payload,
            ..record
        };
        let id = self
            .store
            .write(ctx, idempotency_key, enriched.clone())
            .await?;

        Ok(Record { id, ..enriched })
    }
}
