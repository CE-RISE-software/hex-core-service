use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod auth;
mod dto;
mod error;
mod handlers;
mod metrics;
mod router;

use hex_core::ports::{
    inbound::{record::RecordUseCase, validate::ValidateUseCase},
    outbound::{
        record_store::RecordStorePort, registry::ArtifactRegistryPort, validator::ValidatorPort,
    },
};
use hex_core::usecases::{
    record_usecase::RecordUseCaseImpl, validate_usecase::ValidateUseCaseImpl,
};
use hex_io_http::HttpRecordStore;
use hex_io_memory::MemoryRecordStore;
use hex_registry::catalog_registry::CatalogArtifactRegistry;
use hex_validator_jsonschema::JsonSchemaValidator;
use hex_validator_owl::OwlValidator;
use hex_validator_shacl::ShaclValidator;
use metrics::ApiMetrics;

#[derive(Clone)]
pub struct AppState {
    pub registry: Arc<dyn ArtifactRegistryPort>,
    pub validate_use_case: Arc<dyn ValidateUseCase>,
    pub record_use_case: Arc<dyn RecordUseCase>,
    pub started_at: Instant,
    pub metrics_enabled: bool,
    pub metrics: Arc<ApiMetrics>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_env("LOG_LEVEL").unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer().json())
        .init();

    let host = std::env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let port = std::env::var("SERVER_PORT")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(8080);

    let addr = format!("{host}:{port}");
    let metrics_enabled = std::env::var("METRICS_ENABLED")
        .ok()
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(false);
    let metrics = Arc::new(ApiMetrics::new());

    tracing::info!(%addr, "starting hex-core-service");

    let registry = build_registry_from_env().await?;
    let validators: Vec<Arc<dyn ValidatorPort>> = vec![
        Arc::new(JsonSchemaValidator),
        Arc::new(ShaclValidator::new()),
        Arc::new(OwlValidator::new()),
    ];

    let validate_use_case: Arc<dyn ValidateUseCase> = Arc::new(ValidateUseCaseImpl::new(
        registry.clone(),
        validators.clone(),
    ));

    let io_adapter_id = std::env::var("IO_ADAPTER_ID").unwrap_or_else(|_| "memory".into());
    let store: Arc<dyn RecordStorePort> = match io_adapter_id.as_str() {
        "memory" => Arc::new(MemoryRecordStore::new()),
        "http" => {
            let base_url = std::env::var("IO_ADAPTER_BASE_URL")
                .context("missing IO_ADAPTER_BASE_URL for IO_ADAPTER_ID=http")?;
            let timeout_ms = std::env::var("IO_ADAPTER_TIMEOUT_MS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(5_000);
            Arc::new(HttpRecordStore::new(base_url, timeout_ms))
        }
        other => {
            anyhow::bail!("unsupported IO_ADAPTER_ID={other}; supported values: 'memory', 'http'")
        }
    };

    let record_use_case: Arc<dyn RecordUseCase> = Arc::new(RecordUseCaseImpl {
        registry: registry.clone(),
        validators,
        store,
    });

    let state = Arc::new(AppState {
        registry,
        validate_use_case,
        record_use_case,
        started_at: Instant::now(),
        metrics_enabled,
        metrics,
    });

    let authn = auth::build_provider_from_env()
        .map_err(|e| anyhow::anyhow!("invalid auth configuration: {e:?}"))?;

    let app = router::build(state, authn);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!(%addr, "listening");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn build_registry_from_env() -> Result<Arc<dyn ArtifactRegistryPort>> {
    let registry_mode = std::env::var("REGISTRY_MODE").unwrap_or_else(|_| "catalog".into());
    if registry_mode != "catalog" {
        anyhow::bail!(
            "unsupported REGISTRY_MODE={registry_mode}; only 'catalog' is currently wired in hex-api"
        );
    }

    let allowed_hosts_raw =
        std::env::var("REGISTRY_ALLOWED_HOSTS").context("missing REGISTRY_ALLOWED_HOSTS")?;
    let allowed_hosts = allowed_hosts_raw
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if allowed_hosts.is_empty() {
        anyhow::bail!("REGISTRY_ALLOWED_HOSTS must contain at least one host");
    }

    let require_https = std::env::var("REGISTRY_REQUIRE_HTTPS")
        .context("missing REGISTRY_REQUIRE_HTTPS")?
        .parse::<bool>()
        .context("invalid REGISTRY_REQUIRE_HTTPS (expected true/false)")?;

    let registry = if let Ok(catalog_json) = std::env::var("REGISTRY_CATALOG_JSON") {
        CatalogArtifactRegistry::from_json_catalog(&catalog_json, allowed_hosts, require_https)
            .await
            .context("failed to load REGISTRY_CATALOG_JSON")?
    } else if let Ok(catalog_file) = std::env::var("REGISTRY_CATALOG_FILE") {
        CatalogArtifactRegistry::from_catalog_file(catalog_file, allowed_hosts, require_https)
            .await
            .context("failed to load REGISTRY_CATALOG_FILE")?
    } else if let Ok(catalog_url) = std::env::var("REGISTRY_CATALOG_URL") {
        CatalogArtifactRegistry::from_catalog_url(catalog_url, allowed_hosts, require_https)
            .await
            .context("failed to load REGISTRY_CATALOG_URL")?
    } else {
        anyhow::bail!(
            "missing catalog source: set one of REGISTRY_CATALOG_JSON, REGISTRY_CATALOG_FILE, or REGISTRY_CATALOG_URL"
        );
    };

    Ok(Arc::new(registry))
}
