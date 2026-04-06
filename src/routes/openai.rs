use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};
use tracing::info;

use crate::{
    config::OpenAIConfig,
    error::{GatewayError, GatewayResult},
    providers::Provider,
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/chat/completions", post(chat_completions))
        .route("/models", get(list_models))
}

async fn chat_completions(
    State(state): State<AppState>,
    Json(request): Json<Value>,
) -> GatewayResult<Json<Value>> {
    if state.is_budget_exceeded().await {
        return Err(GatewayError::BudgetExceeded);
    }

    let model = request["model"]
        .as_str()
        .unwrap_or(&state.config.routing.default_provider)
        .to_string();

    info!("Request: model={}", model);

    let target = state
        .router
        .resolve(&model)
        .map_err(|e| GatewayError::UnknownModel(e.to_string()))?;

    info!("Routing to: {}/{}", target.provider, target.model);

    let mut provider_request = request.clone();
    provider_request["model"] = json!(target.model);

    let response = call_provider(&state, &target.provider, provider_request).await?;

    let input_tokens = response["usage"]["prompt_tokens"].as_i64().unwrap_or(0);
    let output_tokens = response["usage"]["completion_tokens"].as_i64().unwrap_or(0);
    let cost = estimate_cost(&target.provider, &target.model, input_tokens, output_tokens);

    state
        .record_spend(
            &target.provider,
            &target.model,
            None,
            input_tokens,
            output_tokens,
            cost,
        )
        .await;

    Ok(Json(response))
}

pub async fn call_provider(
    state: &AppState,
    provider: &str,
    request: Value,
) -> GatewayResult<Value> {
    match provider {
        "anthropic" => {
            let cfg = state
                .config
                .providers
                .anthropic
                .clone()
                .ok_or_else(|| GatewayError::ProviderNotFound("anthropic".into()))?;
            let p = crate::providers::anthropic::AnthropicProvider::new(cfg);
            p.chat(request)
                .await
                .map_err(|e| GatewayError::ProviderError(e.to_string()))
        }
        "openai" => {
            let cfg = state
                .config
                .providers
                .openai
                .clone()
                .ok_or_else(|| GatewayError::ProviderNotFound("openai".into()))?;
            let p = crate::providers::openai::OpenAIProvider::new(cfg);
            p.chat(request)
                .await
                .map_err(|e| GatewayError::ProviderError(e.to_string()))
        }
        "gemini" => {
            let cfg = state
                .config
                .providers
                .gemini
                .clone()
                .ok_or_else(|| GatewayError::ProviderNotFound("gemini".into()))?;
            let p = crate::providers::gemini::GeminiProvider::new(cfg);
            p.chat(request)
                .await
                .map_err(|e| GatewayError::ProviderError(e.to_string()))
        }
        "nvidia" => {
            let cfg = state
                .config
                .providers
                .nvidia
                .clone()
                .ok_or_else(|| GatewayError::ProviderNotFound("nvidia".into()))?;
            let openai_cfg = OpenAIConfig {
                enabled: cfg.enabled,
                api_key: cfg.api_key,
                base_url: cfg.base_url,
                default_model: cfg.default_model,
                available_models: cfg.available_models,
            };
            let p = crate::providers::openai::OpenAIProvider::new(openai_cfg);
            p.chat(request)
                .await
                .map_err(|e| GatewayError::ProviderError(e.to_string()))
        }
        "zai" => {
            let cfg = state
                .config
                .providers
                .zai
                .clone()
                .ok_or_else(|| GatewayError::ProviderNotFound("zai".into()))?;
            let openai_cfg = OpenAIConfig {
                enabled: cfg.enabled,
                api_key: cfg.api_key,
                base_url: cfg.base_url,
                default_model: cfg.default_model,
                available_models: cfg.available_models,
            };
            let p = crate::providers::openai::OpenAIProvider::new(openai_cfg);
            p.chat(request)
                .await
                .map_err(|e| GatewayError::ProviderError(e.to_string()))
        }
        "ollama" => {
            let cfg = state
                .config
                .providers
                .ollama
                .clone()
                .ok_or_else(|| GatewayError::ProviderNotFound("ollama".into()))?;
            let p = crate::providers::ollama::OllamaProvider::new(cfg);
            p.chat(request)
                .await
                .map_err(|e| GatewayError::ProviderError(e.to_string()))
        }
        "inception" => {
            let cfg = state
                .config
                .providers
                .inception
                .clone()
                .ok_or_else(|| GatewayError::ProviderNotFound("inception".into()))?;
            let p = crate::providers::inception::InceptionProvider::new(cfg);
            p.chat(request)
                .await
                .map_err(|e| GatewayError::ProviderError(e.to_string()))
        }
        "openrouter" => {
            let cfg = state
                .config
                .providers
                .openrouter
                .clone()
                .ok_or_else(|| GatewayError::ProviderNotFound("openrouter".into()))?;
            let p = crate::providers::openrouter::OpenRouterProvider::new(cfg);
            p.chat(request)
                .await
                .map_err(|e| GatewayError::ProviderError(e.to_string()))
        }
        other => Err(GatewayError::ProviderNotFound(other.to_string())),
    }
}

async fn list_models(State(state): State<AppState>) -> Json<Value> {
    let mut models = vec![];

    for (alias, target) in &state.config.routing.aliases {
        models.push(json!({
            "id": alias,
            "object": "model",
            "owned_by": "synthia-gateway",
            "routes_to": target
        }));
    }

    if let Some(cfg) = &state.config.providers.anthropic {
        for m in &cfg.available_models {
            models.push(json!({"id": format!("anthropic/{}", m), "object": "model", "owned_by": "anthropic"}));
        }
    }
    if let Some(cfg) = &state.config.providers.openai {
        for m in &cfg.available_models {
            models.push(json!({"id": format!("openai/{}", m), "object": "model", "owned_by": "openai"}));
        }
    }
    if let Some(cfg) = &state.config.providers.gemini {
        for m in &cfg.available_models {
            models.push(json!({"id": format!("gemini/{}", m), "object": "model", "owned_by": "google"}));
        }
    }
    if let Some(cfg) = &state.config.providers.nvidia {
        for m in &cfg.available_models {
            models.push(json!({"id": format!("nvidia/{}", m), "object": "model", "owned_by": "nvidia"}));
        }
    }
    if let Some(cfg) = &state.config.providers.zai {
        for m in &cfg.available_models {
            models.push(json!({"id": format!("zai/{}", m), "object": "model", "owned_by": "zai"}));
        }
    }

    Json(json!({ "object": "list", "data": models }))
}

/// Cost estimate by provider + model for spend tracking (not billing)
pub fn estimate_cost(provider: &str, model: &str, input: i64, output: i64) -> f64 {
    let (input_rate, output_rate) = match (provider, model) {
        ("anthropic", m) if m.contains("opus") => (0.000015, 0.000075),
        ("anthropic", m) if m.contains("sonnet") => (0.000003, 0.000015),
        ("anthropic", m) if m.contains("haiku") => (0.00000025, 0.00000125),
        ("openai", m) if m.contains("gpt-4o") && !m.contains("mini") => (0.0000025, 0.00001),
        ("openai", m) if m.contains("mini") => (0.00000015, 0.0000006),
        ("gemini", m) if m.contains("1.5-pro") => (0.00000125, 0.000005),
        ("gemini", m) if m.contains("flash") => (0.000000075, 0.0000003),
        ("nvidia", _) => (0.000000235, 0.000000235),
        ("ollama", _) => (0.0, 0.0),
        _ => (0.000001, 0.000002),
    };
    (input as f64 * input_rate) + (output as f64 * output_rate)
}
