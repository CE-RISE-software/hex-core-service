use crate::domain::model::{ModelId, ModelVersion};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidatorKind {
    JsonSchema,
    Shacl,
    Owl,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationViolation {
    pub path: Option<String>,
    pub message: String,
    pub severity: Severity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub kind: ValidatorKind,
    pub passed: bool,
    pub violations: Vec<ValidationViolation>,
}

/// Merged outcome of all enabled validators for a single (model, version, payload) triple.
/// `passed` is true only when every individual `ValidationResult` passed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    pub model: ModelId,
    pub version: ModelVersion,
    pub passed: bool,
    pub results: Vec<ValidationResult>,
}

impl ValidationReport {
    pub fn new(model: ModelId, version: ModelVersion, results: Vec<ValidationResult>) -> Self {
        let passed = results.iter().all(|r| r.passed);
        Self {
            model,
            version,
            passed,
            results,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Severity, ValidationReport, ValidationResult, ValidationViolation, ValidatorKind};
    use crate::domain::model::{ModelId, ModelVersion};

    #[test]
    fn validation_report_passed_true_when_all_results_pass() {
        let results = vec![
            ValidationResult {
                kind: ValidatorKind::JsonSchema,
                passed: true,
                violations: vec![],
            },
            ValidationResult {
                kind: ValidatorKind::Shacl,
                passed: true,
                violations: vec![],
            },
        ];

        let report = ValidationReport::new(
            ModelId("product-passport".into()),
            ModelVersion("1.0.0".into()),
            results,
        );
        assert!(report.passed);
    }

    #[test]
    fn validation_report_passed_false_when_any_result_fails() {
        let results = vec![
            ValidationResult {
                kind: ValidatorKind::JsonSchema,
                passed: true,
                violations: vec![],
            },
            ValidationResult {
                kind: ValidatorKind::Shacl,
                passed: false,
                violations: vec![ValidationViolation {
                    path: Some("$.record_scope".into()),
                    message: "invalid value".into(),
                    severity: Severity::Error,
                }],
            },
        ];

        let report = ValidationReport::new(
            ModelId("product-passport".into()),
            ModelVersion("1.0.0".into()),
            results,
        );
        assert!(!report.passed);
    }

    #[test]
    fn validation_report_serde_round_trip() {
        let report = ValidationReport::new(
            ModelId("dp-record".into()),
            ModelVersion("2.1.0".into()),
            vec![ValidationResult {
                kind: ValidatorKind::Owl,
                passed: false,
                violations: vec![ValidationViolation {
                    path: None,
                    message: "ontology mismatch".into(),
                    severity: Severity::Warning,
                }],
            }],
        );

        let json = serde_json::to_string(&report).expect("serialize ValidationReport");
        let decoded: ValidationReport =
            serde_json::from_str(&json).expect("deserialize ValidationReport");

        assert_eq!(decoded.model.0, "dp-record");
        assert_eq!(decoded.version.0, "2.1.0");
        assert_eq!(decoded.results.len(), 1);
        assert!(!decoded.passed);
    }
}
