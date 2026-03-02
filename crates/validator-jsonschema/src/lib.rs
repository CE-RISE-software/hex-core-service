use async_trait::async_trait;
use hex_core::domain::{
    error::ValidatorError,
    model::ArtifactSet,
    validation::{Severity, ValidationResult, ValidationViolation, ValidatorKind},
};
use hex_core::ports::outbound::validator::ValidatorPort;

pub struct JsonSchemaValidator;

#[async_trait]
impl ValidatorPort for JsonSchemaValidator {
    fn kind(&self) -> ValidatorKind {
        ValidatorKind::JsonSchema
    }

    async fn validate(
        &self,
        artifacts: &ArtifactSet,
        payload: &serde_json::Value,
    ) -> Result<ValidationResult, ValidatorError> {
        let schema_text = match &artifacts.schema {
            Some(s) => s,
            None => {
                // No schema artifact present — skip gracefully.
                return Ok(ValidationResult {
                    kind: ValidatorKind::JsonSchema,
                    passed: true,
                    violations: vec![],
                });
            }
        };

        let schema_value: serde_json::Value = serde_json::from_str(schema_text)
            .map_err(|e| ValidatorError::Init(format!("invalid JSON Schema: {e}")))?;

        let compiled = jsonschema::JSONSchema::compile(&schema_value)
            .map_err(|e| ValidatorError::Init(format!("schema compilation failed: {e}")))?;

        let violations: Vec<ValidationViolation> = match compiled.validate(payload) {
            Ok(()) => vec![],
            Err(errors) => errors
                .map(|e| ValidationViolation {
                    path: Some(e.instance_path.to_string()),
                    message: e.to_string(),
                    severity: Severity::Error,
                })
                .collect(),
        };

        let passed = violations.is_empty();

        Ok(ValidationResult {
            kind: ValidatorKind::JsonSchema,
            passed,
            violations,
        })
    }
}
