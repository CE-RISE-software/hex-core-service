use std::sync::Arc;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod auth;
mod dto;
mod error;
mod handlers;
mod router;

use hex_core::ports::{
    inbound::{record::RecordUseCase, validate::ValidateUseCase},
    outbound::registry::ArtifactRegistryPort,
};

#[derive(Clone)]
pub struct AppState {
    pub registry: Arc<dyn ArtifactRegistryPort>,
    pub validate_use_case: Arc<dyn ValidateUseCase>,
    pub record_use_case: Arc<dyn RecordUseCase>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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

    tracing::info!(%addr, "starting hex-core-service");

    // TODO: wire real adapters from IO_ADAPTER_ID / REGISTRY_URL_TEMPLATE env vars.
    // The state construction below is a placeholder; replace with the DI wiring
    // once registry and adapter crates are integrated (Phase 5, AGENTS.md §18).
    panic!("AppState wiring not yet implemented — see AGENTS.md §18 Phase 5");

    #[allow(unreachable_code)]
    {
        let state = Arc::new(AppState {
            registry: todo!(),
            validate_use_case: todo!(),
            record_use_case: todo!(),
        });

        let app = router::build(state);
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        tracing::info!(%addr, "listening");
        axum::serve(listener, app).await?;
        Ok(())
    }
}
