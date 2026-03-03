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

#[cfg(test)]
mod tests {
    use super::EnrichUseCaseImpl;
    use crate::domain::{
        auth::SecurityContext,
        error::{EnricherError, StoreError},
        model::{ModelId, ModelVersion},
        record::{Record, RecordId},
    };
    use crate::ports::{
        inbound::enrich::EnrichUseCase,
        outbound::{enricher::EnricherPort, record_store::RecordStorePort},
    };
    use std::sync::{Arc, Mutex};

    struct StoreStub {
        source_record: Record,
        written_key: Arc<Mutex<Option<String>>>,
        written_payload: Arc<Mutex<Option<serde_json::Value>>>,
        write_id: RecordId,
    }

    #[async_trait::async_trait]
    impl RecordStorePort for StoreStub {
        async fn write(
            &self,
            _ctx: &SecurityContext,
            idempotency_key: &str,
            record: Record,
        ) -> Result<RecordId, StoreError> {
            *self.written_key.lock().expect("lock key") = Some(idempotency_key.to_string());
            *self.written_payload.lock().expect("lock payload") = Some(record.payload);
            Ok(self.write_id.clone())
        }

        async fn read(&self, _ctx: &SecurityContext, _id: &RecordId) -> Result<Record, StoreError> {
            Ok(self.source_record.clone())
        }

        async fn query(
            &self,
            _ctx: &SecurityContext,
            _filter: serde_json::Value,
        ) -> Result<Vec<Record>, StoreError> {
            Ok(vec![])
        }
    }

    struct EnricherStub {
        payload: serde_json::Value,
    }

    #[async_trait::async_trait]
    impl EnricherPort for EnricherStub {
        async fn enrich(
            &self,
            _ctx: &SecurityContext,
            _record: &Record,
        ) -> Result<serde_json::Value, EnricherError> {
            Ok(self.payload.clone())
        }
    }

    fn ctx() -> SecurityContext {
        SecurityContext {
            subject: "tester".into(),
            roles: vec![],
            scopes: vec![],
            tenant: None,
            raw_token: None,
        }
    }

    #[tokio::test]
    async fn enrich_reads_enriches_and_writes_with_idempotency_key() {
        let key_slot = Arc::new(Mutex::new(None));
        let payload_slot = Arc::new(Mutex::new(None));
        let source = Record {
            id: RecordId("original-id".into()),
            model: ModelId("model-a".into()),
            version: ModelVersion("1.0.0".into()),
            payload: serde_json::json!({"old": true}),
        };

        let usecase = EnrichUseCaseImpl {
            store: Arc::new(StoreStub {
                source_record: source,
                written_key: key_slot.clone(),
                written_payload: payload_slot.clone(),
                write_id: RecordId("enriched-id".into()),
            }),
            enricher: Arc::new(EnricherStub {
                payload: serde_json::json!({"new": true}),
            }),
        };

        let enriched = usecase
            .enrich(&ctx(), "idem-enrich-1", &RecordId("original-id".into()))
            .await
            .expect("enrich succeeds");

        assert_eq!(
            key_slot.lock().expect("lock key").as_deref(),
            Some("idem-enrich-1")
        );
        assert_eq!(
            payload_slot.lock().expect("lock payload").as_ref(),
            Some(&serde_json::json!({"new": true}))
        );
        assert_eq!(enriched.id.0, "enriched-id");
        assert_eq!(enriched.payload, serde_json::json!({"new": true}));
    }
}
