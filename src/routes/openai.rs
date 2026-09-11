use axum::{
    extract::State,
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};
use tracing::{info, warn};

use crate::{
    classify::{self, FailureKind},
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

/// One entry in the attempt log attached to every response so a fallback is
/// never silent.
#[derive(Debug, Clone, serde::Serialize)]
struct AttemptRecord {
    provider: String,
    model: String,
    outcome: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    failure: Option<String>,
}

/// What data may leave the building. Requests default to "standard"
/// (potentially confidential); only an explicit tag opens restricted lanes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DataClass {
    Standard,
    NonConfidential,
}

impl DataClass {
    fn as_str(&self) -> &'static str {
        match self {
            DataClass::Standard => "standard",
            DataClass::NonConfidential => "non-confidential",
        }
    }
}

fn data_class_of(headers: &HeaderMap, request: &Value) -> DataClass {
    let raw = headers
        .get("x-data-class")
        .and_then(|v| v.to_str().ok())
        .map(String::from)
        .or_else(|| {
            request["metadata"]["data_class"]
                .as_str()
                .map(String::from)
        });
    match raw.as_deref().map(|s| s.trim().to_ascii_lowercase()) {
        Some(ref s) if s == "non-confidential" => DataClass::NonConfidential,
        _ => DataClass::Standard,
    }
}

async fn chat_completions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<Value>,
) -> GatewayResult<Json<Value>> {
    if state.is_budget_exceeded().await {
        return Err(GatewayError::BudgetExceeded);
    }

    let requested_model = request["model"]
        .as_str()
        .unwrap_or(&state.config.routing.default_provider)
        .to_string();
    let data_class = data_class_of(&headers, &request);
    let agent_id = headers
        .get("x-agent-id")
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    info!("Request: model={} class={}", requested_model, data_class.as_str());

    // Confidentiality gate on the explicit target: asking a training-on-inputs
    // provider to handle confidential work is a hard error, never a silent
    // reroute through the fallback chain.
    let primary = state
        .router
        .resolve(&requested_model)
        .map_err(|e| GatewayError::UnknownModel(e.to_string()))?;
    if state.config.is_restricted_provider(&primary.provider)
        && data_class != DataClass::NonConfidential
    {
        return Err(GatewayError::RestrictedProvider(primary.provider));
    }

    // Build the attempt list: primary target, then the fallback chain.
    let mut targets = vec![primary];
    for entry in &state.config.routing.fallback_chain {
        match state.router.resolve_fallback(entry) {
            Ok(t) => targets.push(t),
            Err(e) => warn!("Skipping fallback entry '{}': {}", entry, e),
        }
    }
    targets.dedup_by(|a, b| a.provider == b.provider && a.model == b.model);

    let mut attempts: Vec<AttemptRecord> = Vec::new();
    let mut last_failure: Option<(FailureKind, String)> = None;

    for target in &targets {
        // Restricted providers inside the chain only serve tagged traffic.
        if state.config.is_restricted_provider(&target.provider)
            && data_class != DataClass::NonConfidential
        {
            attempts.push(AttemptRecord {
                provider: target.provider.clone(),
                model: target.model.clone(),
                outcome: "skipped_restricted".into(),
                failure: None,
            });
            continue;
        }

        // Configured and credentialed?
        if !state.config.provider_enabled(&target.provider) {
            attempts.push(AttemptRecord {
                provider: target.provider.clone(),
                model: target.model.clone(),
                outcome: "skipped_not_configured".into(),
                failure: Some(FailureKind::Unavailable.to_string()),
            });
            last_failure = Some((
                FailureKind::Unavailable,
                format!("Provider '{}' is not configured or disabled", target.provider),
            ));
            continue;
        }
        if !state.config.provider_has_credentials(&target.provider) {
            attempts.push(AttemptRecord {
                provider: target.provider.clone(),
                model: target.model.clone(),
                outcome: "skipped_misconfigured".into(),
                failure: Some(FailureKind::Unavailable.to_string()),
            });
            last_failure = Some((
                FailureKind::Unavailable,
                format!("Provider '{}' is enabled but missing credentials", target.provider),
            ));
            continue;
        }

        // Circuit breaker, at both granularities: provider-wide health and
        // the per-model key used for daily-cap failures (a capped model must
        // not darken its healthy siblings on the same provider).
        let model_circuit_key = format!("{}:{}", target.provider, target.model);
        if !state.circuit_allows(&target.provider).await
            || !state.circuit_allows(&model_circuit_key).await
        {
            attempts.push(AttemptRecord {
                provider: target.provider.clone(),
                model: target.model.clone(),
                outcome: "skipped_circuit_open".into(),
                failure: None,
            });
            continue;
        }

        info!("Routing to: {}/{}", target.provider, target.model);
        let mut provider_request = request.clone();
        provider_request["model"] = json!(target.model);

        match call_provider(&state, &target.provider, provider_request).await {
            Ok(response) => {
                state.circuit_success(&target.provider).await;
                attempts.push(AttemptRecord {
                    provider: target.provider.clone(),
                    model: target.model.clone(),
                    outcome: "success".into(),
                    failure: None,
                });

                let input_tokens = response["usage"]["prompt_tokens"].as_i64().unwrap_or(0);
                let output_tokens =
                    response["usage"]["completion_tokens"].as_i64().unwrap_or(0);
                let cost = crate::pricing::estimate_cost(
                    &target.provider,
                    &target.model,
                    input_tokens,
                    output_tokens,
                );
                state
                    .record_spend(
                        &target.provider,
                        &target.model,
                        agent_id.as_deref(),
                        input_tokens,
                        output_tokens,
                        cost,
                    )
                    .await;

                let fell_back = target.provider != targets[0].provider
                    || target.model != targets[0].model;
                let mut response = response;
                // Honest-routing receipt: which provider/model actually
                // answered and what happened before it. No silent downgrades.
                response["synthia"] = json!({
                    "requested_model": requested_model,
                    "provider": target.provider,
                    "model": target.model,
                    "fell_back": fell_back,
                    "data_class": data_class.as_str(),
                    "cost_usd": cost,
                    "attempts": attempts,
                });
                return Ok(Json(response));
            }
            Err(err) => {
                let kind = classify::classify_error(&err);
                if kind.trips_circuit() {
                    // Daily quotas are per-model: open the model-scoped
                    // circuit so a capped gpt-oss never takes compound down
                    // with it. Everything else is provider health.
                    let key = if kind == FailureKind::RateLimitDaily {
                        format!("{}:{}", target.provider, target.model)
                    } else {
                        target.provider.clone()
                    };
                    state.circuit_failure(&key).await;
                }
                let message = format!("{}", err);
                warn!("{}/{} failed [{}]: {}", target.provider, target.model, kind, message);
                attempts.push(AttemptRecord {
                    provider: target.provider.clone(),
                    model: target.model.clone(),
                    outcome: "failed".into(),
                    failure: Some(kind.to_string()),
                });
                let retryable = kind.should_fallback();
                last_failure = Some((kind, message));
                if !retryable {
                    break;
                }
            }
        }
    }

    let (_kind, message) = last_failure.unwrap_or((FailureKind::Unknown, "no attempt was made".into()));
    Err(GatewayError::AllProvidersFailed {
        requested_model,
        attempts: json!({
            "last_error": message,
            "attempts": attempts,
        }),
    })
}

pub async fn call_provider(
    state: &AppState,
    provider: &str,
    request: Value,
) -> Result<Value, anyhow::Error> {
    match provider {
        "anthropic" => {
            let cfg = state
                .config
                .providers
                .anthropic
                .clone()
                .ok_or_else(|| anyhow::anyhow!("provider 'anthropic' is not configured"))?;
            crate::providers::anthropic::AnthropicProvider::new(cfg).chat(request).await
        }
        "openai" => {
            let cfg = state
                .config
                .providers
                .openai
                .clone()
                .ok_or_else(|| anyhow::anyhow!("provider 'openai' is not configured"))?;
            crate::providers::openai::OpenAIProvider::new(cfg).chat(request).await
        }
        "groq" => {
            let cfg = state
                .config
                .providers
                .groq
                .clone()
                .ok_or_else(|| anyhow::anyhow!("provider 'groq' is not configured"))?;
            crate::providers::groq::GroqProvider::new(cfg).chat(request).await
        }
        "cloudflare" => {
            let cfg = state
                .config
                .providers
                .cloudflare
                .clone()
                .ok_or_else(|| anyhow::anyhow!("provider 'cloudflare' is not configured"))?;
            crate::providers::cloudflare::CloudflareProvider::new(cfg)?
                .chat(request)
                .await
        }
        "gemini" => {
            let cfg = state
                .config
                .providers
                .gemini
                .clone()
                .ok_or_else(|| anyhow::anyhow!("provider 'gemini' is not configured"))?;
            crate::providers::gemini::GeminiProvider::new(cfg).chat(request).await
        }
        "nvidia" => {
            let cfg = state
                .config
                .providers
                .nvidia
                .clone()
                .ok_or_else(|| anyhow::anyhow!("provider 'nvidia' is not configured"))?;
            let openai_cfg = OpenAIConfig {
                enabled: cfg.enabled,
                api_key: cfg.api_key,
                base_url: cfg.base_url,
                default_model: cfg.default_model,
                available_models: cfg.available_models,
            };
            crate::providers::openai::OpenAIProvider::new(openai_cfg).chat(request).await
        }
        "zai" => {
            let cfg = state
                .config
                .providers
                .zai
                .clone()
                .ok_or_else(|| anyhow::anyhow!("provider 'zai' is not configured"))?;
            crate::providers::zai::ZaiProvider::new(cfg).chat(request).await
        }
        "ollama" => {
            let cfg = state
                .config
                .providers
                .ollama
                .clone()
                .ok_or_else(|| anyhow::anyhow!("provider 'ollama' is not configured"))?;
            crate::providers::ollama::OllamaProvider::new(cfg).chat(request).await
        }
        "inception" => {
            let cfg = state
                .config
                .providers
                .inception
                .clone()
                .ok_or_else(|| anyhow::anyhow!("provider 'inception' is not configured"))?;
            crate::providers::inception::InceptionProvider::new(cfg).chat(request).await
        }
        "openrouter" => {
            let cfg = state
                .config
                .providers
                .openrouter
                .clone()
                .ok_or_else(|| anyhow::anyhow!("provider 'openrouter' is not configured"))?;
            crate::providers::openrouter::OpenRouterProvider::new(cfg).chat(request).await
        }
        other => Err(anyhow::anyhow!("provider '{}' is not configured", other)),
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

    let configured: Vec<(&str, Vec<String>)> = vec![
        ("anthropic", state.config.providers.anthropic.as_ref().map(|c| c.available_models.clone()).unwrap_or_default()),
        ("openai", state.config.providers.openai.as_ref().map(|c| c.available_models.clone()).unwrap_or_default()),
        ("gemini", state.config.providers.gemini.as_ref().map(|c| c.available_models.clone()).unwrap_or_default()),
        ("nvidia", state.config.providers.nvidia.as_ref().map(|c| c.available_models.clone()).unwrap_or_default()),
        ("zai", state.config.providers.zai.as_ref().map(|c| c.available_models.clone()).unwrap_or_default()),
        ("groq", state.config.providers.groq.as_ref().map(|c| c.available_models.clone()).unwrap_or_default()),
        ("cloudflare", state.config.providers.cloudflare.as_ref().map(|c| c.available_models.clone()).unwrap_or_default()),
        ("openrouter", state.config.providers.openrouter.as_ref().map(|c| c.available_models.clone()).unwrap_or_default()),
        ("ollama", state.config.providers.ollama.as_ref().map(|c| c.available_models.clone()).unwrap_or_default()),
    ];

    for (provider, available) in configured {
        for m in available {
            models.push(json!({
                "id": format!("{}/{}", provider, m),
                "object": "model",
                "owned_by": provider,
                "free_tier": crate::pricing::is_free(provider, &m),
                "trains_on_inputs": state.config.is_restricted_provider(provider),
            }));
        }
    }

    Json(json!({ "object": "list", "data": models }))
}

// Re-export so the Anthropic passthrough keeps its existing import path.
pub use crate::pricing::estimate_cost;
