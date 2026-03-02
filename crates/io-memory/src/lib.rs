use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use hex_core::domain::{
    auth::SecurityContext,
    error::StoreError,
    record::{Record, RecordId},
};
use hex_core::ports::outbound::record_store::RecordStorePort;

#[derive(Debug)]
struct Entry {
    record: Record,
    idempotency_key: String,
}

/// In-memory implementation of `RecordStorePort`.
/// Intended for tests and local development only — data is lost on restart.
#[derive(Debug, Default, Clone)]
pub struct MemoryRecordStore {
    store: Arc<RwLock<HashMap<String, Entry>>>,
    /// Maps idempotency key → record id to enforce deduplication.
    idempotency_index: Arc<RwLock<HashMap<String, String>>>,
}

impl MemoryRecordStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl RecordStorePort for MemoryRecordStore {
    async fn write(
        &self,
        _ctx: &SecurityContext,
        idempotency_key: &str,
        record: Record,
    ) -> Result<RecordId, StoreError> {
        let mut idx = self.idempotency_index.write().await;

        if let Some(existing_id) = idx.get(idempotency_key) {
            let store = self.store.read().await;
            if let Some(entry) = store.get(existing_id) {
                // Same key, same payload — idempotent success.
                if entry.record.payload == record.payload {
                    return Ok(entry.record.id.clone());
                }
                // Same key, different payload — conflict.
                return Err(StoreError::IdempotencyConflict {
                    key: idempotency_key.to_string(),
                });
            }
        }

        let id = record.id.clone();
        idx.insert(idempotency_key.to_string(), id.0.clone());

        let mut store = self.store.write().await;
        store.insert(
            id.0.clone(),
            Entry {
                record,
                idempotency_key: idempotency_key.to_string(),
            },
        );

        Ok(id)
    }

    async fn read(&self, _ctx: &SecurityContext, id: &RecordId) -> Result<Record, StoreError> {
        let store = self.store.read().await;
        store
            .get(&id.0)
            .map(|e| e.record.clone())
            .ok_or_else(|| StoreError::NotFound { id: id.0.clone() })
    }

    async fn query(
        &self,
        _ctx: &SecurityContext,
        _filter: serde_json::Value,
    ) -> Result<Vec<Record>, StoreError> {
        // TODO: implement filter evaluation; returns all records for now.
        let store = self.store.read().await;
        Ok(store.values().map(|e| e.record.clone()).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hex_core::domain::{
        auth::SecurityContext,
        model::{ModelId, ModelVersion},
        record::{Record, RecordId},
    };

    fn ctx() -> SecurityContext {
        SecurityContext {
            subject: "test-user".into(),
            roles: vec![],
            scopes: vec![],
            tenant: None,
            raw_token: None,
        }
    }

    fn record(id: &str) -> Record {
        Record {
            id: RecordId(id.into()),
            model: ModelId("test-model".into()),
            version: ModelVersion("1.0.0".into()),
            payload: serde_json::json!({ "key": "value" }),
        }
    }

    #[tokio::test]
    async fn write_and_read_round_trip() {
        let store = MemoryRecordStore::new();
        let rec = record("rec-1");
        let ctx = ctx();
        let id = store.write(&ctx, "idem-1", rec.clone()).await.unwrap();
        let fetched = store.read(&ctx, &id).await.unwrap();
        assert_eq!(fetched.id, rec.id);
    }

    #[tokio::test]
    async fn idempotent_write_returns_same_id() {
        let store = MemoryRecordStore::new();
        let ctx = ctx();
        let rec = record("rec-2");
        let id1 = store.write(&ctx, "idem-2", rec.clone()).await.unwrap();
        let id2 = store.write(&ctx, "idem-2", rec.clone()).await.unwrap();
        assert_eq!(id1, id2);
    }

    #[tokio::test]
    async fn conflict_on_different_payload_same_key() {
        let store = MemoryRecordStore::new();
        let ctx = ctx();
        let rec1 = record("rec-3");
        let mut rec2 = record("rec-3");
        rec2.payload = serde_json::json!({ "key": "other" });
        store.write(&ctx, "idem-3", rec1).await.unwrap();
        let err = store.write(&ctx, "idem-3", rec2).await.unwrap_err();
        assert!(matches!(err, StoreError::IdempotencyConflict { .. }));
    }

    #[tokio::test]
    async fn read_not_found() {
        let store = MemoryRecordStore::new();
        let ctx = ctx();
        let err = store
            .read(&ctx, &RecordId("missing".into()))
            .await
            .unwrap_err();
        assert!(matches!(err, StoreError::NotFound { .. }));
    }
}
