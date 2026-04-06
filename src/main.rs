mod config;
mod error;
mod middleware;
mod providers;
mod routes;
mod state;

use axum::{
    middleware as axum_middleware,
    Router,
};
use std::net::SocketAddr;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::{config::GatewayConfig, state::AppState};

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

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        // Health — no auth
        .nest("/health", routes::health::router())
        // OpenAI-compatible endpoints
        .nest("/v1", routes::openai::router())
        // Anthropic-native passthrough
        .nest("/anthropic", routes::anthropic::router())
        // SYNTHIA-specific endpoints
        .nest("/synthia", routes::synthia::router())
        // Admin UI
        .nest("/admin", routes::admin::router())
        // Auth middleware (applied after routing, before handlers)
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            middleware::auth::verify_gateway_key,
        ))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

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
