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
    let names = [
        "anthropic", "openai", "gemini", "nvidia", "inception", "zai", "ollama",
        "openrouter", "groq", "cloudflare", "byokey",
    ];
    let circuits: std::collections::HashMap<String, (bool, u32)> = {
        let cb = state.circuit.read().await;
        cb.snapshot()
            .into_iter()
            .map(|s| (s.provider, (s.open, s.consecutive_failures)))
            .collect()
    };

    let provider_list: Vec<Value> = names
        .iter()
        .map(|name| {
            let enabled = state.config.provider_enabled(name);
            let (circuit_open, consecutive_failures) =
                circuits.get(*name).copied().unwrap_or((false, 0));
            json!({
                "name": name,
                "enabled": enabled,
                "has_credentials": state.config.provider_has_credentials(name),
                "circuit_open": circuit_open,
                "consecutive_failures": consecutive_failures,
                "trains_on_inputs": state.config.is_restricted_provider(name),
            })
        })
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
        "aliases": state.config.routing.aliases,
        "restricted_providers": state.config.safety.trains_on_inputs_providers,
        "circuits": state.circuit.read().await.snapshot()
    }))
}
