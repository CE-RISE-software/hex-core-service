//! OWL validator implementation of `ValidatorPort`.
//!
//! Operational mode: embedded profile checks.
//! No external subprocess engine is required.

use async_trait::async_trait;
use hex_core::domain::{
    error::ValidatorError,
    model::ArtifactSet,
    validation::{Severity, ValidationResult, ValidationViolation, ValidatorKind},
};
use hex_core::ports::outbound::validator::ValidatorPort;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

#[derive(Debug, Clone, Default)]
pub struct OwlValidatorOptions {
    /// Test hook used by contract tests to assert execution failure mapping.
    pub simulate_execution_failure: bool,
}

pub struct OwlValidator {
    options: OwlValidatorOptions,
}

impl OwlValidator {
    pub fn new() -> Self {
        Self {
            options: OwlValidatorOptions::default(),
        }
    }

    pub fn with_options(options: OwlValidatorOptions) -> Self {
        Self { options }
    }
}

impl Default for OwlValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ValidatorPort for OwlValidator {
    fn kind(&self) -> ValidatorKind {
        ValidatorKind::Owl
    }

    async fn validate(
        &self,
        artifacts: &ArtifactSet,
        payload: &serde_json::Value,
    ) -> Result<ValidationResult, ValidatorError> {
        let owl = match &artifacts.owl {
            Some(s) => s,
            None => {
                return Ok(ValidationResult {
                    kind: ValidatorKind::Owl,
                    passed: true,
                    violations: vec![],
                });
            }
        };

        if self.options.simulate_execution_failure {
            return Err(ValidatorError::Execution(
                "simulated OWL validator execution failure".into(),
            ));
        }

        validate_ontology_text(owl)?;

        let mut violations = Vec::new();
        validate_record_scope(payload, &mut violations);
        validate_related_passports(payload, &mut violations);
        validate_metadata_versioning(payload, &mut violations);

        Ok(ValidationResult {
            kind: ValidatorKind::Owl,
            passed: violations.is_empty(),
            violations,
        })
    }
}

fn validate_ontology_text(owl: &str) -> Result<(), ValidatorError> {
    // Keep ontology sanity checks lightweight for the embedded mode.
    if !owl.contains("owl:Ontology") {
        return Err(ValidatorError::Init(
            "invalid OWL artifact: missing owl:Ontology declaration".into(),
        ));
    }
    Ok(())
}

fn validate_record_scope(payload: &serde_json::Value, violations: &mut Vec<ValidationViolation>) {
    if let Some(scope) = payload.get("record_scope") {
        let valid = matches!(scope.as_str(), Some("product" | "material"));
        if !valid {
            push_violation(
                violations,
                "$.record_scope",
                "record_scope must be one of: product, material",
            );
        }
    }
}

fn validate_related_passports(
    payload: &serde_json::Value,
    violations: &mut Vec<ValidationViolation>,
) {
    const ALLOWED: &[&str] = &[
        "derived_from",
        "contributes_to",
        "split_from",
        "merged_into",
        "recycled_into",
        "manufactured_from",
    ];

    if let Some(items) = payload.get("related_passports").and_then(|v| v.as_array()) {
        for (idx, item) in items.iter().enumerate() {
            if let Some(relation_type) = item.get("relation_type") {
                let ok = relation_type
                    .as_str()
                    .map(|v| ALLOWED.contains(&v))
                    .unwrap_or(false);
                if !ok {
                    push_violation(
                        violations,
                        format!("$.related_passports[{idx}].relation_type"),
                        "relation_type is not allowed by OWL profile constraints",
                    );
                }
            }
        }
    }
}

fn validate_metadata_versioning(
    payload: &serde_json::Value,
    violations: &mut Vec<ValidationViolation>,
) {
    let Some(meta) = payload
        .get("metadata_versioning")
        .and_then(|v| v.as_object())
    else {
        return;
    };

    for key in ["metadata_created", "metadata_modified"] {
        if let Some(value) = meta.get(key) {
            let Some(text) = value.as_str() else {
                push_violation(
                    violations,
                    format!("$.metadata_versioning.{key}"),
                    "value must be an RFC3339 date-time string",
                );
                continue;
            };
            if OffsetDateTime::parse(text, &Rfc3339).is_err() {
                push_violation(
                    violations,
                    format!("$.metadata_versioning.{key}"),
                    "value is not a valid RFC3339 date-time",
                );
            }
        }
    }
}

fn push_violation(
    violations: &mut Vec<ValidationViolation>,
    path: impl Into<String>,
    message: impl Into<String>,
) {
    violations.push(ValidationViolation {
        path: Some(path.into()),
        message: message.into(),
        severity: Severity::Error,
    });
}
