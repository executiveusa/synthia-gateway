use std::net::SocketAddr;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use synthia_gateway::{build_router, config::GatewayConfig, state::AppState};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Tracing
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = GatewayConfig::from_env()?;
    info!(
        "SYNTHIA Gateway v{} starting — {} providers active",
        env!("CARGO_PKG_VERSION"),
        config.active_provider_count()
    );

    let state = AppState::new(config).await?;
    let app = build_router(state);

    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "8018".into())
        .parse()
        .unwrap_or(8018);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("Listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
