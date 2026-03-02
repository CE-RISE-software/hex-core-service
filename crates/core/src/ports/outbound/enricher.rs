use async_trait::async_trait;

use crate::domain::{
    auth::SecurityContext,
    error::EnricherError,
    record::Record,
};

#[async_trait]
pub trait EnricherPort: Send + Sync {
    async fn enrich(
        &self,
        ctx: &SecurityContext,
        record: &Record,
    ) -> Result<serde_json::Value, EnricherError>;
}
