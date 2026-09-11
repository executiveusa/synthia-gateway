pub mod circuit;
pub mod classify;
pub mod config;
pub mod error;
pub mod middleware;
pub mod pricing;
pub mod providers;
pub mod routes;
pub mod state;

use axum::{middleware as axum_middleware, Router};
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};

use crate::state::AppState;

/// Build the full gateway router with auth + budget middleware layered on.
pub fn build_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
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
        // Auth first, then the budget gate (outermost layer runs last in
        // axum's onion model, so budget sees only authenticated requests)
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            middleware::spend::check_budget,
        ))
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            middleware::auth::verify_gateway_key,
        ))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
