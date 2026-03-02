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
