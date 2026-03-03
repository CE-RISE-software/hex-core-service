use async_trait::async_trait;
use hex_core::domain::{
    auth::SecurityContext,
    error::StoreError,
    record::{Record, RecordId},
};
use hex_core::ports::outbound::record_store::RecordStorePort;

// Phase 7 scaffold: fields are populated now and consumed once HTTP methods are implemented.
#[allow(dead_code)]
pub struct HttpRecordStore {
    base_url: String,
    timeout_ms: u64,
    client: reqwest::Client,
}

impl HttpRecordStore {
    pub fn new(base_url: impl Into<String>, timeout_ms: u64) -> Self {
        Self {
            base_url: base_url.into(),
            timeout_ms,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl RecordStorePort for HttpRecordStore {
    async fn write(
        &self,
        _ctx: &SecurityContext,
        _idempotency_key: &str,
        _record: Record,
    ) -> Result<RecordId, StoreError> {
        todo!("HTTP IO adapter: write")
    }

    async fn read(&self, _ctx: &SecurityContext, _id: &RecordId) -> Result<Record, StoreError> {
        todo!("HTTP IO adapter: read")
    }

    async fn query(
        &self,
        _ctx: &SecurityContext,
        _filter: serde_json::Value,
    ) -> Result<Vec<Record>, StoreError> {
        todo!("HTTP IO adapter: query")
    }
}
