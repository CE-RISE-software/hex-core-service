use hex_core::domain::{error::ValidatorError, model::ArtifactSet, validation::ValidatorKind};
use hex_core::ports::outbound::validator::ValidatorPort;
use hex_validator_owl::OwlValidator;

const MINIMAL_OWL_TTL: &str = r#"
@prefix owl: <http://www.w3.org/2002/07/owl#> .
@prefix ex: <https://example.org/> .
ex:Ontology a owl:Ontology .
"#;

#[test]
fn validator_kind_is_owl() {
    let validator = OwlValidator::new();
    assert!(matches!(validator.kind(), ValidatorKind::Owl));
}

#[tokio::test]
async fn validate_valid_payload_passes() {
    let validator = OwlValidator::new();
    let artifacts = ArtifactSet {
        owl: Some(MINIMAL_OWL_TTL.to_string()),
        ..Default::default()
    };
    let payload = serde_json::json!({
        "record_scope": "product",
        "metadata_versioning": {
            "metadata_created": "2026-03-03T12:00:00Z",
            "metadata_modified": "2026-03-03T12:00:00Z"
        }
    });

    let result = validator
        .validate(&artifacts, &payload)
        .await
        .expect("validation should succeed");

    assert!(result.passed);
    assert!(result.violations.is_empty());
}

#[tokio::test]
async fn validate_invalid_payload_returns_violations() {
    let validator = OwlValidator::new();
    let artifacts = ArtifactSet {
        owl: Some(MINIMAL_OWL_TTL.to_string()),
        ..Default::default()
    };
    let payload = serde_json::json!({
        "record_scope": "document"
    });

    let result = validator
        .validate(&artifacts, &payload)
        .await
        .expect("validator should produce violations, not execution error");

    assert!(!result.passed);
    assert!(!result.violations.is_empty());
}

#[tokio::test]
async fn validate_missing_owl_artifact_skips_gracefully() {
    let validator = OwlValidator::new();
    let artifacts = ArtifactSet::default();
    let payload = serde_json::json!({ "record_scope": "document" });

    let result = validator
        .validate(&artifacts, &payload)
        .await
        .expect("missing owl.ttl should be treated as skip");

    assert!(result.passed);
    assert!(result.violations.is_empty());
}

#[tokio::test]
async fn validate_invalid_owl_artifact_maps_to_init_error() {
    let validator = OwlValidator::new();
    let artifacts = ArtifactSet {
        owl: Some("@prefix ex: <https://example.org/> .\nex:x ex:y ex:z .".into()),
        ..Default::default()
    };
    let payload = serde_json::json!({ "record_scope": "product" });

    let err = validator
        .validate(&artifacts, &payload)
        .await
        .expect_err("invalid ontology must fail initialization");

    assert!(matches!(err, ValidatorError::Init(_)));
}
