//! SHACL validation tests for the `dp-record-metadata` model.
//!
//! Model:   dp-record-metadata
//! Version: 0.0.2  ← pinned; do not change without updating fixtures and MODEL_VERSION
//!
//! Versioned artifact source (immutable tag):
//!   https://codeberg.org/CE-RISE-models/dp-record-metadata/raw/tag/pages-v0.0.2/generated/
//!
//! These tests exercise the current SHACL validation path implementation.
//!
//! ## What these tests verify
//!
//! SHACL operates on RDF graphs, not raw JSON. The validation pipeline is:
//!
//!   1. Convert JSON payload → RDF (via JSON-LD context or explicit mapping).
//!   2. Load the SHACL shapes graph (`shacl.ttl`).
//!   3. Run the SHACL engine against the data graph.
//!   4. Collect `sh:ValidationResult` nodes from the report.
//!
//! The `dp-record-metadata` SHACL shapes enforce:
//!
//! | Shape target          | Key constraints                                              |
//! |-----------------------|--------------------------------------------------------------|
//! | `dpm:DPRecordMetadata`| `sh:in ("product" "material")` for `dpm:record_scope`       |
//! | `dpm:RelatedPassport` | `sh:in (...)` for `dpm:relation_type`; `sh:maxCount 1`      |
//! | `dpm:AppliedSchema`   | `sh:maxCount 1` on `schema_reference`, `schema_usage`, etc. |
//! | `dpm:CompositionInfo` | `sh:datatype xsd:integer` for `sequence_order`              |
//! | `dpm:SchemaUsage`     | `sh:datatype xsd:float` for `completeness_percentage`       |
//! | `void:Dataset`        | `sh:datatype xsd:dateTime` for `metadata_created/modified`  |
//! | `dcat:Resource`       | `sh:datatype xsd:anyURI` for `schema_url`, `documentation_url` |
//!
//! ## Note on `sh:closed true`
//!
//! All shapes use `sh:closed true`, which means any RDF property not listed
//! in `sh:property` will produce a `sh:Violation`. This is stricter than
//! JSON Schema `additionalProperties: false` because it applies to every
//! named graph node, not just the root document.

use hex_core::domain::{model::ArtifactSet, validation::ValidatorKind};
use hex_core::ports::outbound::validator::ValidatorPort;
use hex_validator_shacl::ShaclValidator;

/// Model version these tests are pinned to.
/// Update fixtures, this constant, and the JSON Schema test file together when bumping.
const MODEL_VERSION: &str = "0.0.2";

/// Immutable raw artifact base URL for the pinned version.
const VERSIONED_BASE_URL: &str =
    "https://codeberg.org/CE-RISE-models/dp-record-metadata/raw/tag/pages-v0.0.2/generated/";

const SHACL_TTL: &str = include_str!("fixtures/dp_record_metadata/shacl.ttl");

fn artifact_set_with_shacl() -> ArtifactSet {
    ArtifactSet {
        shacl: Some(SHACL_TTL.to_string()),
        ..Default::default()
    }
}

fn validator() -> ShaclValidator {
    ShaclValidator::new()
}

// ── Validator kind ────────────────────────────────────────────────────────────

#[test]
fn validator_kind_is_shacl() {
    assert!(matches!(validator().kind(), ValidatorKind::Shacl));
}

#[test]
fn model_version_constant_is_correct() {
    assert_eq!(MODEL_VERSION, "0.0.2");
    assert!(
        VERSIONED_BASE_URL.contains(MODEL_VERSION),
        "VERSIONED_BASE_URL must contain MODEL_VERSION — update both together"
    );
}

#[test]
fn no_shacl_artifact_skips_gracefully() {
    // When no SHACL artifact is present the validator must pass silently.
    let artifacts = ArtifactSet::default();
    let payload = serde_json::json!({ "record_scope": "product" });

    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt
        .block_on(validator().validate(&artifacts, &payload))
        .expect("validator should not error when artifact is absent");

    assert!(result.passed);
    assert!(result.violations.is_empty());
}

// ── Valid payloads ────────────────────────────────────────────────────────────

#[tokio::test]
async fn shacl_valid_minimal_product_passes() {
    // Minimal valid payload: only record_scope present.
    // Expected: sh:conforms true, no sh:ValidationResult nodes.
    let artifacts = artifact_set_with_shacl();
    let payload = serde_json::json!({ "record_scope": "product" });

    let result = validator()
        .validate(&artifacts, &payload)
        .await
        .expect("validator should not error");

    assert!(
        result.passed,
        "minimal valid product should pass SHACL; violations: {:?}",
        result.violations
    );
}

#[tokio::test]
async fn shacl_valid_minimal_material_passes() {
    let artifacts = artifact_set_with_shacl();
    let payload = serde_json::json!({ "record_scope": "material" });

    let result = validator()
        .validate(&artifacts, &payload)
        .await
        .expect("validator should not error");

    assert!(
        result.passed,
        "minimal valid material should pass SHACL; violations: {:?}",
        result.violations
    );
}

#[tokio::test]
async fn shacl_valid_full_product_passes() {
    // Full product record with all optional fields populated.
    let artifacts = artifact_set_with_shacl();
    let payload: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/dp_record_metadata/valid_full_product.json"
    ))
    .unwrap();

    let result = validator()
        .validate(&artifacts, &payload)
        .await
        .expect("validator should not error");

    assert!(
        result.passed,
        "full product record should pass SHACL; violations: {:?}",
        result.violations
    );
}

#[tokio::test]
async fn shacl_valid_full_material_passes() {
    let artifacts = artifact_set_with_shacl();
    let payload: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/dp_record_metadata/valid_full_material.json"
    ))
    .unwrap();

    let result = validator()
        .validate(&artifacts, &payload)
        .await
        .expect("validator should not error");

    assert!(
        result.passed,
        "full material record should pass SHACL; violations: {:?}",
        result.violations
    );
}

#[tokio::test]
async fn shacl_empty_object_passes() {
    // All fields optional at root — empty object must pass.
    let artifacts = artifact_set_with_shacl();
    let payload = serde_json::json!({});

    let result = validator()
        .validate(&artifacts, &payload)
        .await
        .expect("validator should not error");

    assert!(
        result.passed,
        "empty object should pass SHACL (no required fields); violations: {:?}",
        result.violations
    );
}

// ── Invalid payloads ──────────────────────────────────────────────────────────

#[tokio::test]
async fn shacl_invalid_record_scope_fails() {
    // dpm:DPRecordMetadata shape: sh:in ("product" "material") for dpm:record_scope.
    // A value not in that set must produce a sh:Violation with sh:resultSeverity sh:Violation.
    let artifacts = artifact_set_with_shacl();
    let payload = serde_json::json!({ "record_scope": "document" });

    let result = validator()
        .validate(&artifacts, &payload)
        .await
        .expect("validator should not error");

    assert!(
        !result.passed,
        "invalid record_scope value should fail SHACL"
    );
    assert!(
        !result.violations.is_empty(),
        "at least one sh:ValidationResult expected"
    );
    assert!(
        result.violations.iter().any(|v| v
            .path
            .as_deref()
            .map(|p| p.contains("record_scope"))
            .unwrap_or(false)),
        "violation should point to dpm:record_scope path"
    );
}

#[tokio::test]
async fn shacl_invalid_relation_type_fails() {
    // dpm:RelatedPassport shape: sh:in (...) for dpm:relation_type.
    // "transformed_by" is not in the allowed enum.
    let artifacts = artifact_set_with_shacl();
    let payload = serde_json::json!({
        "record_scope": "product",
        "related_passports": [
            {
                "related_passport_id": "https://passports.example.org/material/xyz",
                "relation_type": "transformed_by"
            }
        ]
    });

    let result = validator()
        .validate(&artifacts, &payload)
        .await
        .expect("validator should not error");

    assert!(!result.passed, "invalid relation_type should fail SHACL");
    assert!(!result.violations.is_empty());
}

#[tokio::test]
async fn shacl_invalid_datetime_fails() {
    // void:Dataset shape: sh:datatype xsd:dateTime for dcterms:created.
    // A plain string that is not an ISO 8601 datetime must fail.
    let artifacts = artifact_set_with_shacl();
    let payload = serde_json::json!({
        "record_scope": "product",
        "metadata_versioning": {
            "metadata_created": "not-a-datetime"
        }
    });

    let result = validator()
        .validate(&artifacts, &payload)
        .await
        .expect("validator should not error");

    assert!(
        !result.passed,
        "non-datetime metadata_created should fail SHACL xsd:dateTime constraint"
    );
}

#[tokio::test]
async fn shacl_invalid_sequence_order_type_fails() {
    // dpm:CompositionInfo shape: sh:datatype xsd:integer for dpm:sequence_order.
    // A string value must fail the datatype constraint.
    let artifacts = artifact_set_with_shacl();
    let payload = serde_json::json!({
        "record_scope": "product",
        "applied_schemas": [
            {
                "composition_info": {
                    "sequence_order": "first"
                }
            }
        ]
    });

    let result = validator()
        .validate(&artifacts, &payload)
        .await
        .expect("validator should not error");

    assert!(
        !result.passed,
        "string sequence_order should fail xsd:integer datatype constraint"
    );
}

#[tokio::test]
async fn shacl_invalid_completeness_percentage_type_fails() {
    // dpm:SchemaUsage shape: sh:datatype xsd:float for dpm:completeness_percentage.
    // A string value must fail.
    let artifacts = artifact_set_with_shacl();
    let payload = serde_json::json!({
        "record_scope": "product",
        "applied_schemas": [
            {
                "schema_usage": {
                    "completeness_percentage": "ninety-five"
                }
            }
        ]
    });

    let result = validator()
        .validate(&artifacts, &payload)
        .await
        .expect("validator should not error");

    assert!(
        !result.passed,
        "string completeness_percentage should fail xsd:float datatype constraint"
    );
}

#[tokio::test]
async fn shacl_closed_shape_rejects_extra_properties() {
    // All shapes use sh:closed true.
    // An unknown property inside AppliedSchema must produce a sh:Violation.
    let artifacts = artifact_set_with_shacl();
    let payload = serde_json::json!({
        "record_scope": "product",
        "applied_schemas": [
            {
                "unknown_field": "not allowed by sh:closed true"
            }
        ]
    });

    let result = validator()
        .validate(&artifacts, &payload)
        .await
        .expect("validator should not error");

    assert!(
        !result.passed,
        "unknown property inside sh:closed shape should produce a violation"
    );
}

// ── Violation detail assertions ───────────────────────────────────────────────

#[tokio::test]
async fn shacl_violation_has_path_and_message() {
    // Verify that violations are populated with useful path and message fields,
    // not just a boolean flag — important for API error reporting (§9.4).
    let artifacts = artifact_set_with_shacl();
    let payload = serde_json::json!({ "record_scope": "invalid_scope_value" });

    let result = validator()
        .validate(&artifacts, &payload)
        .await
        .expect("validator should not error");

    assert!(!result.passed);
    let v = result.violations.first().expect("at least one violation");
    assert!(
        v.path.is_some(),
        "violation must carry a path (sh:resultPath)"
    );
    assert!(
        !v.message.is_empty(),
        "violation must carry a human-readable message (sh:resultMessage)"
    );
}
