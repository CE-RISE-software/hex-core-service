use crate::domain::{
    auth::SecurityContext,
    error::StoreError,
    record::{Record, RecordId},
};

#[async_trait::async_trait]
pub trait RecordStorePort: Send + Sync {
    async fn write(
        &self,
        ctx: &SecurityContext,
        idempotency_key: &str,
        record: Record,
    ) -> Result<RecordId, StoreError>;

    async fn read(
        &self,
        ctx: &SecurityContext,
        id: &RecordId,
    ) -> Result<Record, StoreError>;

    async fn query(
        &self,
        ctx: &SecurityContext,
        filter: serde_json::Value,
    ) -> Result<Vec<Record>, StoreError>;
}
