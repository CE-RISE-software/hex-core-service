use std::fmt;

use serde::{Deserialize, Serialize};

/// Identifies a model repository, e.g. `"product-passport"`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ModelId(pub String);

impl fmt::Display for ModelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Version string without leading `v`, e.g. `"1.2.0"`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ModelVersion(pub String);

impl fmt::Display for ModelVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Lightweight descriptor used in registry index listings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelDescriptor {
    pub id: ModelId,
    pub version: ModelVersion,
}

/// All artifacts resolved for a `(model, version)` pair.
/// Fields are `None` when the artifact is absent from the registry.
#[derive(Debug, Clone, Default)]
pub struct ArtifactSet {
    /// Required for dispatch. Absence makes the model non-routable.
    pub route: Option<serde_json::Value>,
    /// JSON Schema text.
    pub schema: Option<String>,
    /// SHACL shapes graph (Turtle).
    pub shacl: Option<String>,
    /// OWL ontology (Turtle).
    pub owl: Option<String>,
    /// OpenAPI document (YAML or JSON text).
    pub openapi: Option<String>,
}

impl ArtifactSet {
    /// Returns `true` only when a route definition is present.
    pub fn is_routable(&self) -> bool {
        self.route.is_some()
    }
}

/// Returned by `ArtifactRegistryPort::refresh`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshSummary {
    /// ISO 8601 timestamp of when the refresh completed.
    pub refreshed_at: String,
    pub models_found: usize,
    /// Per-model errors encountered during resolution (non-fatal).
    pub errors: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_id_display() {
        assert_eq!(
            ModelId("product-passport".into()).to_string(),
            "product-passport"
        );
    }

    #[test]
    fn model_id_serde_round_trip() {
        let id = ModelId("product-passport".into());
        let json = serde_json::to_string(&id).expect("serialize ModelId");
        let decoded: ModelId = serde_json::from_str(&json).expect("deserialize ModelId");
        assert_eq!(decoded.0, "product-passport");
    }

    #[test]
    fn model_version_display_and_serde_round_trip() {
        let version = ModelVersion("1.2.3".into());
        assert_eq!(version.to_string(), "1.2.3");

        let json = serde_json::to_string(&version).expect("serialize ModelVersion");
        let decoded: ModelVersion = serde_json::from_str(&json).expect("deserialize ModelVersion");
        assert_eq!(decoded.0, "1.2.3");
    }

    #[test]
    fn model_descriptor_serde_round_trip() {
        let descriptor = ModelDescriptor {
            id: ModelId("dp-record".into()),
            version: ModelVersion("2.0.0".into()),
        };
        let json = serde_json::to_string(&descriptor).expect("serialize ModelDescriptor");
        let decoded: ModelDescriptor =
            serde_json::from_str(&json).expect("deserialize ModelDescriptor");

        assert_eq!(decoded.id.0, "dp-record");
        assert_eq!(decoded.version.0, "2.0.0");
    }

    #[test]
    fn artifact_set_is_routable_false_by_default() {
        assert!(!ArtifactSet::default().is_routable());
    }

    #[test]
    fn artifact_set_is_routable_true_when_route_present() {
        let mut a = ArtifactSet::default();
        a.route = Some(serde_json::json!({}));
        assert!(a.is_routable());
    }

    #[test]
    fn refresh_summary_serde_round_trip() {
        let summary = RefreshSummary {
            refreshed_at: "2026-03-03T12:00:00Z".into(),
            models_found: 2,
            errors: vec!["model-a@1.0.0: missing route".into()],
        };

        let json = serde_json::to_string(&summary).expect("serialize RefreshSummary");
        let decoded: RefreshSummary =
            serde_json::from_str(&json).expect("deserialize RefreshSummary");

        assert_eq!(decoded.refreshed_at, "2026-03-03T12:00:00Z");
        assert_eq!(decoded.models_found, 2);
        assert_eq!(decoded.errors.len(), 1);
    }
}
