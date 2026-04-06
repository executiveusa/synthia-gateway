use axum::{extract::State, routing::get, Json, Router};
use serde_json::{json, Value};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/spend", get(get_spend))
        .route("/providers", get(get_providers))
        .route("/status", get(get_status))
}

async fn get_spend(State(state): State<AppState>) -> Json<Value> {
    let spend = *state.daily_spend.read().await;
    let budget = state.config.circuit_breaker.daily_budget_usd;
    Json(json!({
        "today_usd": spend,
        "budget_usd": budget,
        "remaining_usd": budget - spend,
        "pct_used": ((spend / budget) * 100.0).round(),
        "halted": spend >= budget
    }))
}

async fn get_providers(State(state): State<AppState>) -> Json<Value> {
    let providers = vec![
        ("anthropic", state.config.providers.anthropic.as_ref().map(|p| p.enabled).unwrap_or(false)),
        ("openai", state.config.providers.openai.as_ref().map(|p| p.enabled).unwrap_or(false)),
        ("gemini", state.config.providers.gemini.as_ref().map(|p| p.enabled).unwrap_or(false)),
        ("nvidia", state.config.providers.nvidia.as_ref().map(|p| p.enabled).unwrap_or(false)),
        ("inception", state.config.providers.inception.as_ref().map(|p| p.enabled).unwrap_or(false)),
        ("zai", state.config.providers.zai.as_ref().map(|p| p.enabled).unwrap_or(false)),
        ("ollama", state.config.providers.ollama.as_ref().map(|p| p.enabled).unwrap_or(false)),
        ("openrouter", state.config.providers.openrouter.as_ref().map(|p| p.enabled).unwrap_or(false)),
        ("byokey", state.config.providers.byokey.as_ref().map(|p| p.enabled).unwrap_or(false)),
    ];

    let provider_list: Vec<Value> = providers
        .iter()
        .map(|(name, enabled)| json!({ "name": name, "enabled": enabled }))
        .collect();

    Json(json!({ "providers": provider_list }))
}

async fn get_status(State(state): State<AppState>) -> Json<Value> {
    let spend = *state.daily_spend.read().await;
    let budget = state.config.circuit_breaker.daily_budget_usd;
    Json(json!({
        "service": "synthia-gateway",
        "version": env!("CARGO_PKG_VERSION"),
        "default_provider": state.config.routing.default_provider,
        "fallback_chain": state.config.routing.fallback_chain,
        "spend_today_usd": spend,
        "budget_usd": budget,
        "active_providers": state.config.active_provider_count(),
        "aliases": state.config.routing.aliases
    }))
}
