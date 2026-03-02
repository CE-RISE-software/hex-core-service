use crate::domain::error::RegistryError;
use crate::domain::model::{ArtifactSet, ModelDescriptor, ModelId, ModelVersion, RefreshSummary};
use async_trait::async_trait;

#[async_trait]
pub trait ArtifactRegistryPort: Send + Sync {
    /// Resolve all available artifacts for a given (model, version) pair.
    async fn resolve(
        &self,
        model: &ModelId,
        version: &ModelVersion,
    ) -> Result<ArtifactSet, RegistryError>;

    /// List all (model, version) pairs currently in the in-memory index.
    async fn list_models(&self) -> Result<Vec<ModelDescriptor>, RegistryError>;

    /// Re-run discovery and atomically swap the in-memory index.
    async fn refresh(&self) -> Result<RefreshSummary, RegistryError>;
}
