//! Integration tests for the dp-record-metadata JSON Schema validator.
//!
//! Model:   dp-record-metadata
//! Version: 0.0.2  ← pinned; do not change without updating fixtures and MODEL_VERSION
//!
//! Versioned artifact source (immutable tag):
//!   https://codeberg.org/CE-RISE-models/dp-record-metadata/raw/tag/pages-v0.0.2/generated/
//!
//! Fixtures are embedded at compile time for offline and CI use.
//! Live fetch tests (feature = "integration-tests") fetch from the immutable
//! tagged URL above, not the floating pages URL, so they are stable across
//! future model releases.

use hex_core::{domain::model::ArtifactSet, ports::outbound::validator::ValidatorPort};
use hex_validator_jsonschema::JsonSchemaValidator;
use serde_json::Value;

/// Model version these tests are pinned to.
/// Update fixtures, this constant, and the SHACL test file together when bumping.
const MODEL_VERSION: &str = "0.0.2";

/// Immutable raw artifact base URL for the pinned version.
const VERSIONED_BASE_URL: &str =
    "https://codeberg.org/CE-RISE-models/dp-record-metadata/raw/tag/pages-v0.0.2/generated/";

// ── Embedded fixtures ─────────────────────────────────────────────────────────

const SCHEMA: &str = include_str!("fixtures/dp_record_metadata/schema.json");

const VALID_EMPTY: &str = include_str!("fixtures/dp_record_metadata/valid_empty.json");
const VALID_MINIMAL: &str = include_str!("fixtures/dp_record_metadata/valid_minimal.json");
const VALID_FULL_PRODUCT: &str =
    include_str!("fixtures/dp_record_metadata/valid_full_product.json");
const VALID_FULL_MATERIAL: &str =
    include_str!("fixtures/dp_record_metadata/valid_full_material.json");
const VALID_EXPLICIT_NULLS: &str =
    include_str!("fixtures/dp_record_metadata/valid_explicit_nulls.json");

const INVALID_BAD_RECORD_SCOPE: &str =
    include_str!("fixtures/dp_record_metadata/invalid_bad_record_scope.json");
const INVALID_BAD_RELATION_TYPE: &str =
    include_str!("fixtures/dp_record_metadata/invalid_bad_relation_type.json");
const INVALID_BAD_DATETIME: &str =
    include_str!("fixtures/dp_record_metadata/invalid_bad_datetime.json");
const INVALID_WRONG_TYPE_COMPLETENESS: &str =
    include_str!("fixtures/dp_record_metadata/invalid_wrong_type_completeness.json");
const INVALID_WRONG_TYPE_SEQUENCE_ORDER: &str =
    include_str!("fixtures/dp_record_metadata/invalid_wrong_type_sequence_order.json");
const INVALID_EXTRA_PROPS_IN_APPLIED_SCHEMA: &str =
    include_str!("fixtures/dp_record_metadata/invalid_extra_props_in_applied_schema.json");

// ── Helpers ───────────────────────────────────────────────────────────────────

fn artifact_set(schema: &str) -> ArtifactSet {
    ArtifactSet {
        schema: Some(schema.to_string()),
        ..Default::default()
    }
}

fn payload(json: &str) -> Value {
    serde_json::from_str(json).expect("fixture is valid JSON")
}

async fn assert_passes(label: &str, schema: &str, instance: &str) {
    let artifacts = artifact_set(schema);
    let result = JsonSchemaValidator
        .validate(&artifacts, &payload(instance))
        .await
        .unwrap_or_else(|e| panic!("[{label}] validator returned Err: {e}"));

    assert!(
        result.passed,
        "[{label}] expected PASS but got violations:\n{:#?}",
        result.violations
    );
}

async fn assert_fails(label: &str, schema: &str, instance: &str) {
    let artifacts = artifact_set(schema);
    let result = JsonSchemaValidator
        .validate(&artifacts, &payload(instance))
        .await
        .unwrap_or_else(|e| panic!("[{label}] validator returned Err: {e}"));

    assert!(
        !result.passed,
        "[{label}] expected FAIL but payload passed validation"
    );
    assert!(
        !result.violations.is_empty(),
        "[{label}] expected at least one violation but list is empty"
    );
}

// ── Valid payloads ────────────────────────────────────────────────────────────

#[tokio::test]
async fn valid_empty_object_passes() {
    // Root schema has no required fields; an empty object must pass.
    assert_passes("valid_empty", SCHEMA, VALID_EMPTY).await;
}

#[tokio::test]
async fn valid_minimal_product_passes() {
    // Minimal payload: only record_scope set to a valid enum value.
    assert_passes("valid_minimal", SCHEMA, VALID_MINIMAL).await;
}

#[tokio::test]
async fn valid_full_product_record_passes() {
    // Fully populated product record with two applied_schemas and two
    // related_passports covering multiple relation_type enum values.
    assert_passes("valid_full_product", SCHEMA, VALID_FULL_PRODUCT).await;
}

#[tokio::test]
async fn valid_full_material_record_passes() {
    // Fully populated material record exercising "material" scope and
    // relation types contributes_to and recycled_into.
    assert_passes("valid_full_material", SCHEMA, VALID_FULL_MATERIAL).await;
}

#[tokio::test]
async fn valid_explicit_nulls_pass() {
    // Optional fields explicitly set to null must pass (anyOf null is allowed).
    assert_passes("valid_explicit_nulls", SCHEMA, VALID_EXPLICIT_NULLS).await;
}

#[tokio::test]
async fn valid_no_schema_artifact_skips_gracefully() {
    // When ArtifactSet has no schema, the validator must skip and return passed=true.
    let empty_artifacts = ArtifactSet::default();
    let result = JsonSchemaValidator
        .validate(
            &empty_artifacts,
            &serde_json::json!({"record_scope": "product"}),
        )
        .await
        .expect("validator must not error when schema is absent");

    assert!(
        result.passed,
        "expected PASS (skip) when no schema artifact is present, got violations: {:#?}",
        result.violations
    );
    assert!(
        result.violations.is_empty(),
        "expected no violations when skipping, got: {:#?}",
        result.violations
    );
}

#[tokio::test]
async fn valid_extra_property_at_root_passes() {
    // Root schema has additionalProperties: true — extra fields at the root level
    // must not cause a validation failure.
    let instance = serde_json::json!({
        "record_scope": "product",
        "extra_top_level_field": "this is allowed at root"
    });
    let artifacts = artifact_set(SCHEMA);
    let result = JsonSchemaValidator
        .validate(&artifacts, &instance)
        .await
        .expect("validator must not error");

    assert!(
        result.passed,
        "extra property at root should be allowed (additionalProperties: true), got: {:#?}",
        result.violations
    );
}

#[tokio::test]
async fn valid_all_relation_types_accepted() {
    // Exercise every value in PassportRelationTypeEnum.
    let relation_types = [
        "derived_from",
        "contributes_to",
        "split_from",
        "merged_into",
        "recycled_into",
        "manufactured_from",
    ];

    for rt in relation_types {
        let instance = serde_json::json!({
            "record_scope": "product",
            "related_passports": [
                {
                    "related_passport_id": "https://example.org/passport/1",
                    "relation_type": rt
                }
            ]
        });
        let artifacts = artifact_set(SCHEMA);
        let result = JsonSchemaValidator
            .validate(&artifacts, &instance)
            .await
            .unwrap_or_else(|e| panic!("[relation_type={rt}] validator error: {e}"));

        assert!(
            result.passed,
            "[relation_type={rt}] expected PASS but got violations: {:#?}",
            result.violations
        );
    }
}

#[tokio::test]
async fn valid_both_record_scopes_accepted() {
    for scope in ["product", "material"] {
        let instance = serde_json::json!({ "record_scope": scope });
        let artifacts = artifact_set(SCHEMA);
        let result = JsonSchemaValidator
            .validate(&artifacts, &instance)
            .await
            .unwrap_or_else(|e| panic!("[record_scope={scope}] validator error: {e}"));

        assert!(
            result.passed,
            "[record_scope={scope}] expected PASS but got: {:#?}",
            result.violations
        );
    }
}

// ── Invalid payloads ──────────────────────────────────────────────────────────

#[tokio::test]
async fn invalid_bad_record_scope_fails() {
    // "document" is not in RecordScopeEnum ["product", "material"].
    assert_fails("invalid_bad_record_scope", SCHEMA, INVALID_BAD_RECORD_SCOPE).await;
}

#[tokio::test]
async fn invalid_bad_relation_type_fails() {
    // "transformed_by" is not in PassportRelationTypeEnum.
    assert_fails(
        "invalid_bad_relation_type",
        SCHEMA,
        INVALID_BAD_RELATION_TYPE,
    )
    .await;
}

#[tokio::test]
async fn invalid_wrong_type_completeness_percentage_fails() {
    // completeness_percentage must be ["number", "null"], not a string.
    assert_fails(
        "invalid_wrong_type_completeness",
        SCHEMA,
        INVALID_WRONG_TYPE_COMPLETENESS,
    )
    .await;
}

#[tokio::test]
async fn invalid_wrong_type_sequence_order_fails() {
    // sequence_order must be ["integer", "null"], not a string.
    assert_fails(
        "invalid_wrong_type_sequence_order",
        SCHEMA,
        INVALID_WRONG_TYPE_SEQUENCE_ORDER,
    )
    .await;
}

#[tokio::test]
async fn invalid_extra_property_inside_applied_schema_fails() {
    // AppliedSchema has additionalProperties: false — unknown fields must be rejected.
    assert_fails(
        "invalid_extra_props_in_applied_schema",
        SCHEMA,
        INVALID_EXTRA_PROPS_IN_APPLIED_SCHEMA,
    )
    .await;
}

#[tokio::test]
async fn invalid_bad_datetime_fails() {
    // metadata_created has format: "date-time" — a free-form string must fail
    // when format validation is enabled in the jsonschema compiler.
    //
    // Note: jsonschema 0.18 validates built-in formats (date-time, uri, etc.)
    // by default. If this test fails unexpectedly, verify that format
    // validation is not disabled in JsonSchemaValidator compilation options.
    assert_fails("invalid_bad_datetime", SCHEMA, INVALID_BAD_DATETIME).await;
}

#[tokio::test]
async fn invalid_applied_schemas_wrong_type_fails() {
    // applied_schemas must be ["array", "null"]; passing an object must fail.
    let instance = serde_json::json!({
        "record_scope": "product",
        "applied_schemas": { "not": "an array" }
    });
    let artifacts = artifact_set(SCHEMA);
    let result = JsonSchemaValidator
        .validate(&artifacts, &instance)
        .await
        .expect("validator must not error");

    assert!(
        !result.passed,
        "applied_schemas as object should fail, but passed"
    );
}

#[tokio::test]
async fn invalid_schema_compilation_error_is_reported() {
    // Passing a syntactically invalid schema must return Err (not a panic).
    let bad_schema = r#"{ "type": "object", "properties": { "#; // truncated JSON
    let artifacts = artifact_set(bad_schema);
    let result = JsonSchemaValidator
        .validate(&artifacts, &serde_json::json!({}))
        .await;

    assert!(
        result.is_err(),
        "expected Err for invalid schema but got Ok"
    );
}

// ── Embedded fixture version guard ───────────────────────────────────────────

#[test]
fn embedded_schema_version_is_pinned_version() {
    // Fails immediately if the schema.json fixture is updated without bumping
    // MODEL_VERSION, or vice-versa — keeping the constant and file in sync.
    let schema: serde_json::Value = serde_json::from_str(SCHEMA).expect("fixture is valid JSON");
    let fixture_version = schema
        .get("version")
        .and_then(|v| v.as_str())
        .expect("schema fixture must have a 'version' field");

    assert_eq!(
        fixture_version, MODEL_VERSION,
        "schema.json fixture declares version '{fixture_version}' but MODEL_VERSION \
         is '{MODEL_VERSION}' — update one to match the other"
    );
}

// ── Live fetch tests (integration-tests feature only) ────────────────────────

#[cfg(feature = "integration-tests")]
mod live {
    use super::*;

    /// Fetches the schema from the immutable versioned tag on Codeberg.
    /// This URL will never change for v0.0.2, unlike the floating pages URL.
    async fn fetch_versioned_schema() -> String {
        let url = format!("{}schema.json", VERSIONED_BASE_URL);
        reqwest::get(&url)
            .await
            .unwrap_or_else(|e| panic!("failed to reach versioned registry URL {url}: {e}"))
            .text()
            .await
            .expect("failed to read schema body")
    }

    #[tokio::test]
    async fn live_versioned_schema_is_reachable_and_valid_json() {
        let text = fetch_versioned_schema().await;
        let _: serde_json::Value =
            serde_json::from_str(&text).expect("versioned schema is not valid JSON");
    }

    #[tokio::test]
    async fn live_versioned_schema_reports_correct_version() {
        // The artifact at the tagged URL must declare exactly MODEL_VERSION.
        // If this fails the tag was re-published with different content.
        let text = fetch_versioned_schema().await;
        let schema: serde_json::Value = serde_json::from_str(&text).unwrap();
        let version = schema
            .get("version")
            .and_then(|v| v.as_str())
            .expect("live schema must have a 'version' field");

        assert_eq!(
            version, MODEL_VERSION,
            "live versioned schema declares version '{version}' but tests are \
             pinned to MODEL_VERSION '{MODEL_VERSION}'"
        );
    }

    #[tokio::test]
    async fn live_versioned_schema_matches_embedded_fixture() {
        // Structural diff between the live artifact and the embedded fixture.
        // A mismatch means the fixture needs to be refreshed from the tagged URL.
        let live_text = fetch_versioned_schema().await;
        let live: serde_json::Value = serde_json::from_str(&live_text).unwrap();
        let embedded: serde_json::Value = serde_json::from_str(SCHEMA).unwrap();

        assert_eq!(
            live, embedded,
            "embedded schema.json fixture does not match the live artifact at \
             {VERSIONED_BASE_URL}schema.json — re-fetch and update the fixture"
        );
    }

    #[tokio::test]
    async fn live_versioned_schema_accepts_valid_full_product() {
        let schema = fetch_versioned_schema().await;
        assert_passes("live/valid_full_product", &schema, VALID_FULL_PRODUCT).await;
    }

    #[tokio::test]
    async fn live_versioned_schema_rejects_bad_record_scope() {
        let schema = fetch_versioned_schema().await;
        assert_fails(
            "live/invalid_bad_record_scope",
            &schema,
            INVALID_BAD_RECORD_SCOPE,
        )
        .await;
    }

    #[tokio::test]
    async fn live_versioned_schema_rejects_bad_relation_type() {
        let schema = fetch_versioned_schema().await;
        assert_fails(
            "live/invalid_bad_relation_type",
            &schema,
            INVALID_BAD_RELATION_TYPE,
        )
        .await;
    }
}
