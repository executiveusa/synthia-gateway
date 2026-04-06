// Anthropic-native endpoint — Claude Code and Anthropic SDK point directly here

use axum::{extract::State, routing::post, Json, Router};
use serde_json::Value;
use crate::{error::GatewayResult, state::AppState};

pub fn router() -> Router<AppState> {
    Router::new().route("/v1/messages", post(messages))
}

async fn messages(
    State(state): State<AppState>,
    Json(request): Json<Value>,
) -> GatewayResult<Json<Value>> {
    if state.is_budget_exceeded().await {
        return Err(crate::error::GatewayError::BudgetExceeded);
    }

    let cfg = state
        .config
        .providers
        .anthropic
        .clone()
        .ok_or_else(|| crate::error::GatewayError::ProviderNotFound("anthropic".into()))?;

    let base_url = cfg.base_url.clone();
    let api_key = cfg.api_key.clone().unwrap_or_default();

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| crate::error::GatewayError::ProviderError(e.to_string()))?;

    let url = format!("{}/v1/messages", base_url);

    let response = client
        .post(&url)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .header("x-api-key", &api_key)
        .json(&request)
        .send()
        .await
        .map_err(|e| crate::error::GatewayError::ProviderError(e.to_string()))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(crate::error::GatewayError::ProviderError(
            format!("Anthropic error {}: {}", status, body)
        ));
    }

    let json: Value = response
        .json()
        .await
        .map_err(|e| crate::error::GatewayError::ProviderError(e.to_string()))?;

    // Track spend
    let input_tokens = json["usage"]["input_tokens"].as_i64().unwrap_or(0);
    let output_tokens = json["usage"]["output_tokens"].as_i64().unwrap_or(0);
    let model = json["model"].as_str().unwrap_or("unknown");
    let cost = crate::routes::openai::estimate_cost("anthropic", model, input_tokens, output_tokens);
    state
        .record_spend("anthropic", model, None, input_tokens, output_tokens, cost)
        .await;

    Ok(Json(json))
}
