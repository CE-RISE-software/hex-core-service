use std::sync::Arc;

use crate::domain::{
    auth::SecurityContext,
    error::CoreError,
    model::{ModelId, ModelVersion},
    validation::ValidationReport,
};
use crate::ports::{
    inbound::validate::ValidateUseCase,
    outbound::{registry::ArtifactRegistryPort, validator::ValidatorPort},
};

pub struct ValidateUseCaseImpl {
    registry: Arc<dyn ArtifactRegistryPort>,
    validators: Vec<Arc<dyn ValidatorPort>>,
}

impl ValidateUseCaseImpl {
    pub fn new(
        registry: Arc<dyn ArtifactRegistryPort>,
        validators: Vec<Arc<dyn ValidatorPort>>,
    ) -> Self {
        Self {
            registry,
            validators,
        }
    }
}

#[async_trait::async_trait]
impl ValidateUseCase for ValidateUseCaseImpl {
    async fn validate(
        &self,
        _ctx: &SecurityContext,
        model: &ModelId,
        version: &ModelVersion,
        payload: &serde_json::Value,
    ) -> Result<ValidationReport, CoreError> {
        // 1. Resolve artifact set
        let artifacts = self.registry.resolve(model, version).await?;

        // 2. Assert routable
        if !artifacts.is_routable() {
            return Err(CoreError::NotRoutable);
        }

        // 3. Run each validator; skip if its required artifact is absent
        let mut results = Vec::new();
        for validator in &self.validators {
            let result = validator
                .validate(&artifacts, payload)
                .await
                .map_err(CoreError::Validator)?;
            results.push(result);
        }

        // 4. Merge into a single report
        Ok(ValidationReport::new(
            model.clone(),
            version.clone(),
            results,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::ValidateUseCaseImpl;
    use crate::domain::{
        auth::SecurityContext,
        error::{RegistryError, ValidatorError},
        model::{ArtifactSet, ModelId, ModelVersion, RefreshSummary},
        validation::{ValidationResult, ValidatorKind},
    };
    use crate::ports::{
        inbound::validate::ValidateUseCase,
        outbound::{registry::ArtifactRegistryPort, validator::ValidatorPort},
    };
    use std::sync::Arc;

    struct RegistryStub {
        artifacts: ArtifactSet,
    }

    #[async_trait::async_trait]
    impl ArtifactRegistryPort for RegistryStub {
        async fn resolve(
            &self,
            _model: &ModelId,
            _version: &ModelVersion,
        ) -> Result<ArtifactSet, RegistryError> {
            Ok(self.artifacts.clone())
        }

        async fn list_models(
            &self,
        ) -> Result<Vec<crate::domain::model::ModelDescriptor>, RegistryError> {
            Ok(vec![])
        }

        async fn refresh(&self) -> Result<RefreshSummary, RegistryError> {
            Ok(RefreshSummary {
                refreshed_at: "2026-03-03T00:00:00Z".into(),
                models_found: 0,
                errors: vec![],
            })
        }
    }

    struct ValidatorStub {
        result: Option<ValidationResult>,
        error: Option<String>,
    }

    #[async_trait::async_trait]
    impl ValidatorPort for ValidatorStub {
        fn kind(&self) -> ValidatorKind {
            ValidatorKind::JsonSchema
        }

        async fn validate(
            &self,
            _artifacts: &ArtifactSet,
            _payload: &serde_json::Value,
        ) -> Result<ValidationResult, ValidatorError> {
            if let Some(err) = &self.error {
                return Err(ValidatorError::Execution(err.clone()));
            }
            Ok(self.result.clone().expect("validator result configured"))
        }
    }

    fn ctx() -> SecurityContext {
        SecurityContext {
            subject: "tester".into(),
            roles: vec![],
            scopes: vec![],
            tenant: None,
            raw_token: None,
        }
    }

    #[tokio::test]
    async fn validate_returns_merged_report() {
        let usecase = ValidateUseCaseImpl::new(
            Arc::new(RegistryStub {
                artifacts: ArtifactSet {
                    route: Some(serde_json::json!({"op":"validate"})),
                    ..Default::default()
                },
            }),
            vec![
                Arc::new(ValidatorStub {
                    result: Some(ValidationResult {
                        kind: ValidatorKind::JsonSchema,
                        passed: true,
                        violations: vec![],
                    }),
                    error: None,
                }),
                Arc::new(ValidatorStub {
                    result: Some(ValidationResult {
                        kind: ValidatorKind::Shacl,
                        passed: false,
                        violations: vec![],
                    }),
                    error: None,
                }),
            ],
        );

        let report = usecase
            .validate(
                &ctx(),
                &ModelId("model-a".into()),
                &ModelVersion("1.0.0".into()),
                &serde_json::json!({"x":1}),
            )
            .await
            .expect("validate succeeds");

        assert_eq!(report.results.len(), 2);
        assert!(!report.passed);
    }

    #[tokio::test]
    async fn validate_fails_when_not_routable() {
        let usecase = ValidateUseCaseImpl::new(
            Arc::new(RegistryStub {
                artifacts: ArtifactSet::default(),
            }),
            vec![],
        );

        let err = usecase
            .validate(
                &ctx(),
                &ModelId("model-a".into()),
                &ModelVersion("1.0.0".into()),
                &serde_json::json!({"x":1}),
            )
            .await
            .expect_err("must fail");

        assert!(matches!(err, crate::domain::error::CoreError::NotRoutable));
    }

    #[tokio::test]
    async fn validate_propagates_validator_errors() {
        let usecase = ValidateUseCaseImpl::new(
            Arc::new(RegistryStub {
                artifacts: ArtifactSet {
                    route: Some(serde_json::json!({"op":"validate"})),
                    ..Default::default()
                },
            }),
            vec![Arc::new(ValidatorStub {
                result: None,
                error: Some("boom".into()),
            })],
        );

        let err = usecase
            .validate(
                &ctx(),
                &ModelId("model-a".into()),
                &ModelVersion("1.0.0".into()),
                &serde_json::json!({"x":1}),
            )
            .await
            .expect_err("must fail");

        assert!(matches!(err, crate::domain::error::CoreError::Validator(_)));
    }
}
