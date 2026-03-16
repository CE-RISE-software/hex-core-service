use std::sync::Arc;
use std::time::Instant;

use axum::{
    body::Body,
    extract::DefaultBodyLimit,
    http::Request,
    middleware,
    routing::{get, post},
    Router,
};
use axum::{extract::State, middleware::Next, response::Response};

use crate::auth::{self, AuthProviderHandle};
use crate::handlers::{admin, models, operations};
use crate::AppState;

pub fn build(state: Arc<AppState>, authn: AuthProviderHandle) -> Router {
    let request_max_bytes = std::env::var("SERVER_REQUEST_MAX_BYTES")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(1_048_576);

    let protected = Router::new()
        // ── Model operations ──────────────────────────────────────────────────
        .route(
            "/models/{model}/versions/{version_op}",
            post(operations::dispatch),
        )
        .route(
            "/models/{model}/versions/{version}/{operation}",
            post(operations::dispatch_slash),
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
        .route("/admin/ready", get(admin::ready))
        .route("/admin/status", get(admin::status))
        .route("/admin/metrics", get(admin::metrics))
        .route("/admin/registry/refresh", post(admin::registry_refresh))
        .route("/admin/config", get(admin::config))
        .route("/admin/cache/clear", post(admin::cache_clear))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            track_and_log_metrics,
        ))
        .layer(DefaultBodyLimit::max(request_max_bytes))
        .layer(middleware::from_fn_with_state(authn, auth::require_auth));

    Router::new()
        .route("/admin/health", get(admin::health))
        .route("/admin/version", get(admin::version))
        .route("/admin/models/count", get(admin::models_count))
        .merge(protected)
        .with_state(state)
}

async fn track_and_log_metrics(
    State(state): State<Arc<AppState>>,
    req: Request<Body>,
    next: Next,
) -> Response {
    state.metrics.on_request_start();

    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let auth_header = if req.headers().contains_key("authorization") {
        "<redacted>"
    } else {
        "<none>"
    };
    let started = Instant::now();
    tracing::info!(%method, %path, authorization = %auth_header, "request_started");

    let response = next.run(req).await;
    let duration_ms = started.elapsed().as_millis() as u64;
    let status = response.status().as_u16();
    state.metrics.on_request_end(status, duration_ms);
    tracing::info!(%method, %path, status, duration_ms, "request_finished");

    response
}

#[cfg(test)]
mod tests {
    use crate::metrics::ApiMetrics;
    use crate::AppState;
    use axum::{
        extract::{Extension, Path, State},
        http::HeaderMap,
        routing::{get, post},
        Router,
    };
    use hex_core::domain::auth::SecurityContext;
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
    use hex_validator_owl::OwlValidator;
    use hex_validator_shacl::ShaclValidator;
    use std::{path::Path as FsPath, sync::Arc};
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

    async fn write_catalog(path: &FsPath, json: &str) {
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

    fn test_security_context() -> SecurityContext {
        SecurityContext {
            subject: "test-user".into(),
            roles: vec![],
            scopes: vec![],
            tenant: None,
            raw_token: Some("test-token".into()),
        }
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
      "route_url": "{}/CE-RISE-models/model-a/src/tag/pages-v1.0.0/generated/route.json"
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
            started_at: std::time::Instant::now(),
            metrics_enabled: false,
            metrics: Arc::new(ApiMetrics::new()),
        });
        let app = Router::new()
            .route("/models", get(crate::handlers::models::list))
            .route(
                "/admin/models/count",
                get(crate::handlers::admin::models_count),
            )
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

        let count = http
            .get(format!("{base_url}/admin/models/count"))
            .send()
            .await
            .expect("request /admin/models/count");
        assert_eq!(count.status(), 200);
        let count_json: serde_json::Value = count.json().await.expect("json body");
        assert_eq!(count_json["models_count"], 1);

        let catalog_b = format!(
            r#"{{
  "models": [
    {{
      "model": "model-b",
      "version": "2.0.0",
      "route_url": "{}/CE-RISE-models/model-b/src/tag/pages-v2.0.0/generated/route.json"
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

        let count = http
            .get(format!("{base_url}/admin/models/count"))
            .send()
            .await
            .expect("request /admin/models/count");
        assert_eq!(count.status(), 200);
        let count_json: serde_json::Value = count.json().await.expect("json body");
        assert_eq!(count_json["models_count"], 1);

        server_handle.abort();
        let _ = tokio::fs::remove_file(&catalog_path).await;
    }

    #[tokio::test]
    async fn validate_endpoint_executes_shacl_with_real_dp_record_metadata_shapes() {
        let server = MockServer::start().await;
        let route = ResponseTemplate::new(200).set_body_string(r#"{"op":"validate"}"#);
        let shacl = ResponseTemplate::new(200).set_body_string(include_str!(
            "../../validator-shacl/tests/fixtures/dp_record_metadata/shacl.ttl"
        ));
        let not_found = ResponseTemplate::new(404);

        let model = "dp-record-metadata";
        let version = "0.0.2";
        let base = format!("/CE-RISE-models/{model}/src/tag/pages-v{version}/generated");

        Mock::given(method("GET"))
            .and(path(format!("{base}/route.json")))
            .respond_with(route)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!("{base}/shacl.ttl")))
            .respond_with(shacl)
            .mount(&server)
            .await;
        // Keep JSON Schema absent so SHACL is the effective validator for this test.
        for filename in ["schema.json", "owl.ttl", "openapi.json"] {
            Mock::given(method("GET"))
                .and(path(format!("{base}/{filename}")))
                .respond_with(not_found.clone())
                .mount(&server)
                .await;
        }

        let catalog = format!(
            r#"{{
  "models": [
    {{
      "model": "{model}",
      "version": "{version}",
      "route_url": "{}/CE-RISE-models/{model}/src/tag/pages-v{version}/generated/route.json",
      "shacl_url": "{}/CE-RISE-models/{model}/src/tag/pages-v{version}/generated/shacl.ttl"
    }}
  ]
}}"#,
            server.uri(),
            server.uri()
        );
        let registry = CatalogArtifactRegistry::from_json_catalog(&catalog, vec![], false)
            .await
            .expect("registry init");
        let registry: Arc<dyn ArtifactRegistryPort> = Arc::new(registry);

        let validators: Vec<Arc<dyn ValidatorPort>> = vec![
            Arc::new(JsonSchemaValidator),
            Arc::new(ShaclValidator::new()),
        ];
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
            started_at: std::time::Instant::now(),
            metrics_enabled: false,
            metrics: Arc::new(ApiMetrics::new()),
        });
        let response = crate::handlers::operations::validate(
            State(state),
            Path(crate::handlers::operations::ModelPath {
                model: model.to_string(),
                version: version.to_string(),
            }),
            Some(Extension(test_security_context())),
            HeaderMap::new(),
            axum::Json(crate::handlers::operations::ValidateRequest {
                payload: serde_json::json!({ "record_scope": "document" }),
            }),
        )
        .await
        .expect("validate handler should succeed");

        assert!(!response.passed);
        let results = response.results.as_array().expect("results array");
        let shacl_result = results
            .iter()
            .find(|r| r["kind"] == "Shacl")
            .expect("SHACL validator result must be present");
        assert_eq!(shacl_result["passed"], false);
        assert!(
            shacl_result["violations"]
                .as_array()
                .expect("violations array")
                .iter()
                .any(|v| v["path"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("record_scope")),
            "expected SHACL violation on record_scope"
        );
    }

    #[tokio::test]
    async fn validate_endpoint_executes_owl_when_owl_artifact_is_present() {
        let server = MockServer::start().await;
        let route = ResponseTemplate::new(200).set_body_string(r#"{"op":"validate"}"#);
        let owl = ResponseTemplate::new(200).set_body_string(
            r#"
@prefix owl: <http://www.w3.org/2002/07/owl#> .
@prefix ex: <https://example.org/> .
ex:Ontology a owl:Ontology .
"#,
        );
        let not_found = ResponseTemplate::new(404);

        let model = "owl-test-model";
        let version = "1.0.0";
        let base = format!("/CE-RISE-models/{model}/src/tag/pages-v{version}/generated");

        Mock::given(method("GET"))
            .and(path(format!("{base}/route.json")))
            .respond_with(route)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!("{base}/owl.ttl")))
            .respond_with(owl)
            .mount(&server)
            .await;
        for filename in ["schema.json", "shacl.ttl", "openapi.json"] {
            Mock::given(method("GET"))
                .and(path(format!("{base}/{filename}")))
                .respond_with(not_found.clone())
                .mount(&server)
                .await;
        }

        let catalog = format!(
            r#"{{
  "models": [
    {{
      "model": "{model}",
      "version": "{version}",
      "route_url": "{}/CE-RISE-models/{model}/src/tag/pages-v{version}/generated/route.json",
      "owl_url": "{}/CE-RISE-models/{model}/src/tag/pages-v{version}/generated/owl.ttl"
    }}
  ]
}}"#,
            server.uri(),
            server.uri()
        );
        let registry = CatalogArtifactRegistry::from_json_catalog(&catalog, vec![], false)
            .await
            .expect("registry init");
        let registry: Arc<dyn ArtifactRegistryPort> = Arc::new(registry);

        let validators: Vec<Arc<dyn ValidatorPort>> = vec![
            Arc::new(JsonSchemaValidator),
            Arc::new(ShaclValidator::new()),
            Arc::new(OwlValidator::new()),
        ];
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
            started_at: std::time::Instant::now(),
            metrics_enabled: false,
            metrics: Arc::new(ApiMetrics::new()),
        });
        let response = crate::handlers::operations::validate(
            State(state),
            Path(crate::handlers::operations::ModelPath {
                model: model.to_string(),
                version: version.to_string(),
            }),
            Some(Extension(test_security_context())),
            HeaderMap::new(),
            axum::Json(crate::handlers::operations::ValidateRequest {
                payload: serde_json::json!({ "record_scope": "document" }),
            }),
        )
        .await
        .expect("validate handler should succeed");

        assert!(!response.passed);
        let results = response.results.as_array().expect("results array");
        let owl_result = results
            .iter()
            .find(|r| r["kind"] == "Owl")
            .expect("OWL validator result must be present");
        assert_eq!(owl_result["passed"], false);
        assert!(
            owl_result["violations"]
                .as_array()
                .expect("violations array")
                .iter()
                .any(|v| v["path"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("record_scope")),
            "expected OWL violation on record_scope"
        );
    }
}
