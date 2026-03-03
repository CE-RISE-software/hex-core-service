use std::sync::Arc;

use crate::domain::{
    auth::SecurityContext,
    error::CoreError,
    model::{ModelId, ModelVersion},
    record::Record,
};
use crate::ports::{
    inbound::record::RecordUseCase,
    outbound::{
        record_store::RecordStorePort, registry::ArtifactRegistryPort, validator::ValidatorPort,
    },
};

pub struct RecordUseCaseImpl {
    pub registry: Arc<dyn ArtifactRegistryPort>,
    pub validators: Vec<Arc<dyn ValidatorPort>>,
    pub store: Arc<dyn RecordStorePort>,
}

#[async_trait::async_trait]
impl RecordUseCase for RecordUseCaseImpl {
    async fn create(
        &self,
        ctx: &SecurityContext,
        idempotency_key: &str,
        model: &ModelId,
        version: &ModelVersion,
        payload: serde_json::Value,
    ) -> Result<Record, CoreError> {
        let artifacts = self.registry.resolve(model, version).await?;

        if !artifacts.is_routable() {
            return Err(CoreError::NotRoutable);
        }

        // Run all validators; collect results.
        let mut results = Vec::new();
        for v in &self.validators {
            let result = v
                .validate(&artifacts, &payload)
                .await
                .map_err(CoreError::Validator)?;
            results.push(result);
        }

        let report = crate::domain::validation::ValidationReport::new(
            model.clone(),
            version.clone(),
            results,
        );

        if !report.passed {
            return Err(CoreError::ValidationFailed(report));
        }

        let id = crate::domain::record::RecordId(uuid());
        let record = Record {
            id: id.clone(),
            model: model.clone(),
            version: version.clone(),
            payload,
        };

        self.store
            .write(ctx, idempotency_key, record.clone())
            .await?;

        Ok(record)
    }

    async fn query(
        &self,
        ctx: &SecurityContext,
        model: &ModelId,
        version: &ModelVersion,
        filter: serde_json::Value,
    ) -> Result<Vec<Record>, CoreError> {
        let artifacts = self.registry.resolve(model, version).await?;

        if !artifacts.is_routable() {
            return Err(CoreError::NotRoutable);
        }

        Ok(self.store.query(ctx, filter).await?)
    }
}

fn uuid() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    // Minimal ID generation for scaffolding — replace with `uuid` crate in production.
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    format!("record-{nanos:08x}")
}

#[cfg(test)]
mod tests {
    use super::RecordUseCaseImpl;
    use crate::domain::{
        auth::SecurityContext,
        error::{RegistryError, StoreError, ValidatorError},
        model::{ArtifactSet, ModelId, ModelVersion, RefreshSummary},
        record::{Record, RecordId},
        validation::{ValidationResult, ValidatorKind},
    };
    use crate::ports::{
        inbound::record::RecordUseCase,
        outbound::{
            record_store::RecordStorePort, registry::ArtifactRegistryPort, validator::ValidatorPort,
        },
    };
    use std::sync::{Arc, Mutex};

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

    struct StoreStub {
        last_idempotency_key: Arc<Mutex<Option<String>>>,
        last_filter: Arc<Mutex<Option<serde_json::Value>>>,
        write_id: RecordId,
        query_records: Vec<Record>,
    }

    #[async_trait::async_trait]
    impl RecordStorePort for StoreStub {
        async fn write(
            &self,
            _ctx: &SecurityContext,
            idempotency_key: &str,
            _record: Record,
        ) -> Result<RecordId, StoreError> {
            *self
                .last_idempotency_key
                .lock()
                .expect("lock idempotency key") = Some(idempotency_key.to_string());
            Ok(self.write_id.clone())
        }

        async fn read(&self, _ctx: &SecurityContext, _id: &RecordId) -> Result<Record, StoreError> {
            Err(StoreError::NotFound {
                id: "unused".into(),
            })
        }

        async fn query(
            &self,
            _ctx: &SecurityContext,
            filter: serde_json::Value,
        ) -> Result<Vec<Record>, StoreError> {
            *self.last_filter.lock().expect("lock filter") = Some(filter);
            Ok(self.query_records.clone())
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

    fn pass_result() -> ValidationResult {
        ValidationResult {
            kind: ValidatorKind::JsonSchema,
            passed: true,
            violations: vec![],
        }
    }

    #[tokio::test]
    async fn create_propagates_idempotency_key_to_store() {
        let key_slot = Arc::new(Mutex::new(None));
        let store = Arc::new(StoreStub {
            last_idempotency_key: key_slot.clone(),
            last_filter: Arc::new(Mutex::new(None)),
            write_id: RecordId("written-id".into()),
            query_records: vec![],
        });
        let usecase = RecordUseCaseImpl {
            registry: Arc::new(RegistryStub {
                artifacts: ArtifactSet {
                    route: Some(serde_json::json!({"op":"create"})),
                    ..Default::default()
                },
            }),
            validators: vec![Arc::new(ValidatorStub {
                result: Some(pass_result()),
                error: None,
            })],
            store,
        };

        let record = usecase
            .create(
                &ctx(),
                "idem-123",
                &ModelId("model-a".into()),
                &ModelVersion("1.0.0".into()),
                serde_json::json!({"a": 1}),
            )
            .await
            .expect("create succeeds");

        assert_eq!(
            key_slot.lock().expect("lock idempotency key").as_deref(),
            Some("idem-123")
        );
        assert_eq!(record.model.0, "model-a");
        assert_eq!(record.version.0, "1.0.0");
        assert_eq!(record.payload["a"], 1);
    }

    #[tokio::test]
    async fn create_returns_validation_failed_when_validator_fails() {
        let usecase = RecordUseCaseImpl {
            registry: Arc::new(RegistryStub {
                artifacts: ArtifactSet {
                    route: Some(serde_json::json!({"op":"create"})),
                    ..Default::default()
                },
            }),
            validators: vec![Arc::new(ValidatorStub {
                result: Some(ValidationResult {
                    kind: ValidatorKind::JsonSchema,
                    passed: false,
                    violations: vec![],
                }),
                error: None,
            })],
            store: Arc::new(StoreStub {
                last_idempotency_key: Arc::new(Mutex::new(None)),
                last_filter: Arc::new(Mutex::new(None)),
                write_id: RecordId("unused".into()),
                query_records: vec![],
            }),
        };

        let err = usecase
            .create(
                &ctx(),
                "idem-1",
                &ModelId("model-a".into()),
                &ModelVersion("1.0.0".into()),
                serde_json::json!({"a": 1}),
            )
            .await
            .expect_err("must fail");

        assert!(matches!(
            err,
            crate::domain::error::CoreError::ValidationFailed(_)
        ));
    }

    #[tokio::test]
    async fn create_propagates_validator_execution_errors() {
        let usecase = RecordUseCaseImpl {
            registry: Arc::new(RegistryStub {
                artifacts: ArtifactSet {
                    route: Some(serde_json::json!({"op":"create"})),
                    ..Default::default()
                },
            }),
            validators: vec![Arc::new(ValidatorStub {
                result: None,
                error: Some("boom".into()),
            })],
            store: Arc::new(StoreStub {
                last_idempotency_key: Arc::new(Mutex::new(None)),
                last_filter: Arc::new(Mutex::new(None)),
                write_id: RecordId("unused".into()),
                query_records: vec![],
            }),
        };

        let err = usecase
            .create(
                &ctx(),
                "idem-1",
                &ModelId("model-a".into()),
                &ModelVersion("1.0.0".into()),
                serde_json::json!({"a": 1}),
            )
            .await
            .expect_err("must fail");

        assert!(matches!(err, crate::domain::error::CoreError::Validator(_)));
    }

    #[tokio::test]
    async fn query_passes_filter_to_store() {
        let filter_slot = Arc::new(Mutex::new(None));
        let usecase = RecordUseCaseImpl {
            registry: Arc::new(RegistryStub {
                artifacts: ArtifactSet {
                    route: Some(serde_json::json!({"op":"query"})),
                    ..Default::default()
                },
            }),
            validators: vec![],
            store: Arc::new(StoreStub {
                last_idempotency_key: Arc::new(Mutex::new(None)),
                last_filter: filter_slot.clone(),
                write_id: RecordId("unused".into()),
                query_records: vec![],
            }),
        };

        let filter = serde_json::json!({"status":"active"});
        let _ = usecase
            .query(
                &ctx(),
                &ModelId("model-a".into()),
                &ModelVersion("1.0.0".into()),
                filter.clone(),
            )
            .await
            .expect("query succeeds");

        assert_eq!(
            filter_slot.lock().expect("lock filter").as_ref(),
            Some(&filter)
        );
    }
}
