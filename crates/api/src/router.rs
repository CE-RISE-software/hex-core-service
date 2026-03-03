use std::sync::Arc;

use axum::{
    routing::{get, post},
    Router,
};

use crate::handlers::{admin, models, operations};
use crate::AppState;

pub fn build(state: Arc<AppState>) -> Router {
    Router::new()
        // ── Model operations ──────────────────────────────────────────────────
        .route(
            "/models/{model}/versions/{version}:{operation}",
            post(operations::dispatch),
        )
        // ── Public introspection ──────────────────────────────────────────────
        .route("/models", get(models::list))
        .route(
            "/models/{model}/versions/{version}/schema",
            get(models::artifact_schema),
        )
        .route(
            "/models/{model}/versions/{version}/shacl",
            get(models::artifact_shacl),
        )
        .route(
            "/models/{model}/versions/{version}/owl",
            get(models::artifact_owl),
        )
        .route(
            "/models/{model}/versions/{version}/route",
            get(models::artifact_route),
        )
        // ── OpenAPI self-description ──────────────────────────────────────────
        .route("/openapi.json", get(models::openapi_spec))
        // ── Admin ─────────────────────────────────────────────────────────────
        .route("/admin/health", get(admin::health))
        .route("/admin/ready", get(admin::ready))
        .route("/admin/status", get(admin::status))
        .route("/admin/metrics", get(admin::metrics))
        .route("/admin/registry/refresh", post(admin::registry_refresh))
        .route("/admin/config", get(admin::config))
        .route("/admin/cache/clear", post(admin::cache_clear))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use crate::AppState;
    use axum::{
        routing::{get, post},
        Router,
    };
    use hex_core::{
        ports::{
            inbound::{record::RecordUseCase, validate::ValidateUseCase},
            outbound::{
                record_store::RecordStorePort, registry::ArtifactRegistryPort,
                validator::ValidatorPort,
            },
        },
        usecases::{record_usecase::RecordUseCaseImpl, validate_usecase::ValidateUseCaseImpl},
    };
    use hex_io_memory::MemoryRecordStore;
    use hex_registry::catalog_registry::CatalogArtifactRegistry;
    use hex_validator_jsonschema::JsonSchemaValidator;
    use std::{path::Path, sync::Arc};
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer, ResponseTemplate,
    };

    async fn spawn_http(app: Router) -> (String, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test listener");
        let addr = listener.local_addr().expect("local addr");
        let handle = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve app");
        });
        (format!("http://{addr}"), handle)
    }

    async fn write_catalog(path: &Path, json: &str) {
        tokio::fs::write(path, json)
            .await
            .expect("write catalog file");
    }

    fn unique_catalog_path() -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("hex-core-catalog-{nanos}.json"))
    }

    #[tokio::test]
    async fn admin_refresh_reloads_models_from_catalog_file() {
        let server = MockServer::start().await;
        let route = ResponseTemplate::new(200).set_body_string(r#"{"op":"create"}"#);
        let not_found = ResponseTemplate::new(404);

        for (model, version) in [("model-a", "1.0.0"), ("model-b", "2.0.0")] {
            Mock::given(method("GET"))
                .and(path(format!(
                    "/CE-RISE-models/{model}/src/tag/pages-v{version}/generated/route.json"
                )))
                .respond_with(route.clone())
                .mount(&server)
                .await;

            for filename in ["schema.json", "shacl.ttl", "owl.ttl", "openapi.json"] {
                Mock::given(method("GET"))
                    .and(path(format!(
                        "/CE-RISE-models/{model}/src/tag/pages-v{version}/generated/{filename}"
                    )))
                    .respond_with(not_found.clone())
                    .mount(&server)
                    .await;
            }
        }

        let catalog_path = unique_catalog_path();
        let catalog_a = format!(
            r#"{{
  "models": [
    {{
      "model": "model-a",
      "version": "1.0.0",
      "base_url": "{}/CE-RISE-models/model-a/src/tag/pages-v1.0.0/generated/"
    }}
  ]
}}"#,
            server.uri()
        );
        write_catalog(&catalog_path, &catalog_a).await;

        let registry = CatalogArtifactRegistry::from_catalog_file(&catalog_path, vec![], false)
            .await
            .expect("registry init");
        let registry: Arc<dyn ArtifactRegistryPort> = Arc::new(registry);

        let validators: Vec<Arc<dyn ValidatorPort>> = vec![Arc::new(JsonSchemaValidator)];
        let validate_use_case: Arc<dyn ValidateUseCase> = Arc::new(ValidateUseCaseImpl::new(
            registry.clone(),
            validators.clone(),
        ));
        let store: Arc<dyn RecordStorePort> = Arc::new(MemoryRecordStore::new());
        let record_use_case: Arc<dyn RecordUseCase> = Arc::new(RecordUseCaseImpl {
            registry: registry.clone(),
            validators,
            store,
        });

        let state = Arc::new(AppState {
            registry,
            validate_use_case,
            record_use_case,
        });
        let app = Router::new()
            .route("/models", get(crate::handlers::models::list))
            .route(
                "/admin/registry/refresh",
                post(crate::handlers::admin::registry_refresh),
            )
            .with_state(state);
        let (base_url, server_handle) = spawn_http(app).await;

        let http = reqwest::Client::new();

        let response = http
            .get(format!("{base_url}/models"))
            .send()
            .await
            .expect("request /models");
        assert_eq!(response.status(), 200);
        let json: serde_json::Value = response.json().await.expect("json body");
        assert_eq!(json["models"][0]["id"], "model-a");
        assert_eq!(json["models"][0]["version"], "1.0.0");

        let catalog_b = format!(
            r#"{{
  "models": [
    {{
      "model": "model-b",
      "version": "2.0.0",
      "base_url": "{}/CE-RISE-models/model-b/src/tag/pages-v2.0.0/generated/"
    }}
  ]
}}"#,
            server.uri()
        );
        write_catalog(&catalog_path, &catalog_b).await;

        let refresh = http
            .post(format!("{base_url}/admin/registry/refresh"))
            .send()
            .await
            .expect("request /admin/registry/refresh");
        assert_eq!(refresh.status(), 200);

        let response = http
            .get(format!("{base_url}/models"))
            .send()
            .await
            .expect("request /models");
        assert_eq!(response.status(), 200);
        let json: serde_json::Value = response.json().await.expect("json body");
        assert_eq!(json["models"][0]["id"], "model-b");
        assert_eq!(json["models"][0]["version"], "2.0.0");

        server_handle.abort();
        let _ = tokio::fs::remove_file(&catalog_path).await;
    }
}
