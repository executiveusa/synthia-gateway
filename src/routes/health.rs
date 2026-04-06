use axum::{extract::State, routing::get, Json, Router};
use serde_json::{json, Value};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(health_check))
        .route("/ready", get(readiness_check))
}

async fn health_check() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "synthia-gateway",
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

async fn readiness_check(
    State(state): State<AppState>,
) -> Json<Value> {
    let spend = *state.daily_spend.read().await;
    let budget = state.config.circuit_breaker.daily_budget_usd;
    Json(json!({
        "status": "ready",
        "spend_today_usd": spend,
        "budget_usd": budget,
        "budget_remaining_usd": budget - spend,
        "budget_pct_used": (spend / budget * 100.0).round()
    }))
}
