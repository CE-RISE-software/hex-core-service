use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

use hex_core::domain::model::{
    ArtifactSet, ModelDescriptor, ModelId, ModelVersion, RefreshSummary,
};

/// Thread-safe in-memory index of resolved (model, version) → ArtifactSet.
/// Swapped atomically on refresh via write lock.
#[derive(Debug, Default, Clone)]
pub struct RegistryIndex {
    inner: Arc<RwLock<IndexInner>>,
}

#[derive(Debug, Default)]
struct IndexInner {
    entries: HashMap<(ModelId, ModelVersion), ArtifactSet>,
}

impl RegistryIndex {
    pub fn new() -> Self {
        Self::default()
    }

    /// Read a resolved ArtifactSet for the given (model, version).
    pub async fn get(&self, model: &ModelId, version: &ModelVersion) -> Option<ArtifactSet> {
        let guard = self.inner.read().await;
        guard
            .entries
            .get(&(model.clone(), version.clone()))
            .cloned()
    }

    /// List all currently indexed (model, version) descriptors.
    pub async fn list(&self) -> Vec<ModelDescriptor> {
        let guard = self.inner.read().await;
        guard
            .entries
            .keys()
            .map(|(id, ver)| ModelDescriptor {
                id: id.clone(),
                version: ver.clone(),
            })
            .collect()
    }

    /// Atomically replace the entire index with a new set of entries.
    pub async fn swap(
        &self,
        new_entries: HashMap<(ModelId, ModelVersion), ArtifactSet>,
    ) -> RefreshSummary {
        let models_found = new_entries.len();
        let mut guard = self.inner.write().await;
        guard.entries = new_entries;
        RefreshSummary {
            refreshed_at: chrono_now(),
            models_found,
            errors: vec![],
        }
    }

    /// Atomically replace the index and record per-model errors from resolution.
    pub async fn swap_with_errors(
        &self,
        new_entries: HashMap<(ModelId, ModelVersion), ArtifactSet>,
        errors: Vec<String>,
    ) -> RefreshSummary {
        let models_found = new_entries.len();
        let mut guard = self.inner.write().await;
        guard.entries = new_entries;
        RefreshSummary {
            refreshed_at: chrono_now(),
            models_found,
            errors,
        }
    }
}

fn chrono_now() -> String {
    // TODO: replace with chrono or time crate once added to workspace dependencies.
    // Using std as a zero-dependency placeholder for now.
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| format!("{}", d.as_secs()))
        .unwrap_or_else(|_| "unknown".into())
}
