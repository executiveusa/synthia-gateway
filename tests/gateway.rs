//! End-to-end gateway tests: real axum app + mock upstream providers on
//! ephemeral localhost ports. No real provider is ever contacted.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::{
    body::Body,
    extract::State as AxumState,
    http::{HeaderMap, Request, StatusCode},
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use serde_json::{json, Value};
use tower::ServiceExt;

use synthia_gateway::config::{
    AuthConfig, CircuitBreakerConfig, CloudflareConfig, GatewayConfig, GeminiConfig, GroqConfig,
    ProvidersConfig, RoutingConfig, SafetyConfig, ZaiConfig,
};
use synthia_gateway::state::AppState;

const GW_KEY: &str = "test-gateway-key";

// ---------------------------------------------------------------- mock ----

#[derive(Clone, Default)]
struct MockUpstream {
    /// model -> (status, body)
    responses: Arc<HashMap<String, (u16, Value)>>,
    /// every request the mock received: (uri, headers, body)
    seen: Arc<Mutex<Vec<(String, HeaderMap, Value)>>>,
}

async fn mock_handler(
    AxumState(mock): AxumState<MockUpstream>,
    uri: axum::http::Uri,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let uri_s = uri.to_string();
    mock.seen
        .lock()
        .unwrap()
        .push((uri_s.clone(), headers, body.clone()));
    // OpenAI-format upstreams name the model in the JSON body; Gemini names
    // it in the path (/v1beta/models/<model>:generateContent).
    let model = body["model"]
        .as_str()
        .map(String::from)
        .or_else(|| {
            uri_s.split("/v1beta/models/").nth(1).map(|rest| {
                rest.split(':').next().unwrap_or("").to_string()
            })
        })
        .unwrap_or_default();
    match mock.responses.get(&model) {
        Some((status, body)) => (StatusCode::from_u16(*status).unwrap(), Json(body.clone())),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": {"message": format!("no mock for model {}", model)}})),
        ),
    }
}

async fn spawn_mock(responses: HashMap<String, (u16, Value)>) -> (String, MockUpstream) {
    let mock = MockUpstream {
        responses: Arc::new(responses),
        seen: Arc::new(Mutex::new(vec![])),
    };
    let app = Router::new()
        .route("/v1/chat/completions", post(mock_handler))
        .route("/v1beta/models/*rest", post(mock_handler))
        .with_state(mock.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{}", addr), mock)
}

fn ok_completion(model: &str) -> Value {
    json!({
        "id": "chatcmpl-mock",
        "object": "chat.completion",
        "model": model,
        "choices": [{"index": 0, "message": {"role": "assistant", "content": "hi"}, "finish_reason": "stop"}],
        "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
    })
}

fn groq_tpd_body() -> Value {
    json!({"error": {"message": "Rate limit reached for model `openai/gpt-oss-120b` in organization `org_123` on tokens per day (TPD): Limit 200000, Used 196639, Requested 6716. Please try again in 24m9s.", "type": "tokens", "code": "rate_limit_exceeded"}})
}

// ---------------------------------------------------------------- setup ---

fn base_config() -> GatewayConfig {
    GatewayConfig {
        auth: AuthConfig {
            gateway_key: GW_KEY.into(),
            allowed_keys: vec![],
        },
        routing: RoutingConfig {
            default_provider: "groq".into(),
            fallback_chain: vec![],
            aliases: HashMap::new(),
        },
        circuit_breaker: CircuitBreakerConfig {
            daily_budget_usd: 10.0,
            failure_threshold: 2,
            reset_seconds: 600,
        },
        safety: SafetyConfig::default(),
        providers: ProvidersConfig::default(),
    }
}

fn groq_cfg(base_url: &str) -> GroqConfig {
    GroqConfig {
        enabled: true,
        api_key: Some("gsk_test_secret_key".into()),
        base_url: base_url.into(),
        default_model: "openai/gpt-oss-120b".into(),
        available_models: vec![
            "openai/gpt-oss-120b".into(),
            "groq/compound".into(),
        ],
    }
}

async fn make_app(config: GatewayConfig) -> Router {
    let state = AppState::new_with_db_url(config, "sqlite::memory:")
        .await
        .expect("test state");
    synthia_gateway::build_router(state)
}

fn authed(body: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/v1/chat/completions")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", GW_KEY))
        .body(Body::from(body.to_string()))
        .unwrap()
}

async fn json_body(resp: axum::response::Response) -> (StatusCode, Value) {
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or(json!({"raw": String::from_utf8_lossy(&bytes)})))
}

fn chat_req(model: &str) -> Value {
    json!({"model": model, "messages": [{"role": "user", "content": "hello"}]})
}

// ---------------------------------------------------------------- tests ---

/// The core StarNet scenario: gpt-oss-120b hits the Groq daily token cap
/// (429 TPD) and the gateway falls back to groq/compound — with the whole
/// path visible in the response.
#[tokio::test]
async fn daily_cap_429_falls_back_to_compound() {
    let mut responses = HashMap::new();
    responses.insert("openai/gpt-oss-120b".to_string(), (429, groq_tpd_body()));
    responses.insert("groq/compound".to_string(), (200, ok_completion("groq/compound")));
    let (url, _mock) = spawn_mock(responses).await;

    let mut config = base_config();
    config.providers.groq = Some(groq_cfg(&url));
    config.routing.fallback_chain = vec!["groq/groq/compound".into()];
    let app = make_app(config).await;

    let (status, body) = json_body(app.oneshot(authed(chat_req("groq/openai/gpt-oss-120b"))).await.unwrap()).await;

    assert_eq!(status, StatusCode::OK, "body: {}", body);
    assert_eq!(body["choices"][0]["message"]["content"], "hi");
    let syn = &body["synthia"];
    assert_eq!(syn["requested_model"], "groq/openai/gpt-oss-120b");
    assert_eq!(syn["model"], "groq/compound");
    assert_eq!(syn["fell_back"], true);
    assert_eq!(syn["cost_usd"], 0.0);
    let attempts = syn["attempts"].as_array().unwrap();
    assert_eq!(attempts.len(), 2);
    assert_eq!(attempts[0]["outcome"], "failed");
    assert_eq!(attempts[0]["failure"], "rate_limit_daily");
    assert_eq!(attempts[1]["outcome"], "success");
}

/// A 401 is not a reason to try the next provider with a different key —
/// the chain stops and the error surfaces.
#[tokio::test]
async fn bad_request_stops_the_chain() {
    let mut responses = HashMap::new();
    responses.insert(
        "openai/gpt-oss-120b".to_string(),
        (400, json!({"error": {"message": "messages is required"}})),
    );
    responses.insert("groq/compound".to_string(), (200, ok_completion("groq/compound")));
    let (url, mock) = spawn_mock(responses).await;

    let mut config = base_config();
    config.providers.groq = Some(groq_cfg(&url));
    config.routing.fallback_chain = vec!["groq/groq/compound".into()];
    let app = make_app(config).await;

    let (status, body) = json_body(app.oneshot(authed(chat_req("groq/openai/gpt-oss-120b"))).await.unwrap()).await;

    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(body["error"]["code"], "all_providers_failed");
    // compound must NOT have been tried — a bad request fails everywhere.
    assert_eq!(mock.seen.lock().unwrap().len(), 1);
}

/// Unknown provider prefixes are rejected outright: no silent rerouting to
/// the default provider.
#[tokio::test]
async fn unknown_provider_is_rejected_not_rerouted() {
    let mut responses = HashMap::new();
    responses.insert("openai/gpt-oss-120b".to_string(), (200, ok_completion("openai/gpt-oss-120b")));
    let (url, _mock) = spawn_mock(responses).await;

    let mut config = base_config();
    config.providers.groq = Some(groq_cfg(&url));
    let app = make_app(config).await;

    let (status, body) = json_body(app.oneshot(authed(chat_req("nosuchprovider/foo"))).await.unwrap()).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    let attempts = body["error"]["attempts"]["attempts"].as_array().unwrap();
    assert_eq!(attempts[0]["outcome"], "skipped_not_configured");
    assert_eq!(attempts[0]["provider"], "nosuchprovider");
}

/// Two consecutive 500s open the circuit; the next request skips the broken
/// provider entirely and the fallback serves it.
#[tokio::test]
async fn circuit_opens_and_fallback_serves() {
    let mut broken = HashMap::new();
    broken.insert("openai/gpt-oss-120b".to_string(), (500, json!({"error": "boom"})));
    let (broken_url, broken_mock) = spawn_mock(broken).await;

    let mut healthy = HashMap::new();
    healthy.insert("gemini-2.0-flash".to_string(), (200, json!({
        "candidates": [{"content": {"parts": [{"text": "gemini says hi"}]}}],
        "usageMetadata": {"promptTokenCount": 3, "candidatesTokenCount": 2, "totalTokenCount": 5}
    })));
    let (gemini_url, _g) = spawn_mock(healthy).await;

    let mut config = base_config();
    config.providers.groq = Some(groq_cfg(&broken_url));
    config.providers.gemini = Some(GeminiConfig {
        enabled: true,
        api_key: Some("gemini-test-key".into()),
        base_url: gemini_url,
        default_model: "gemini-2.0-flash".into(),
        available_models: vec!["gemini-2.0-flash".into()],
    });
    config.routing.fallback_chain = vec!["gemini/gemini-2.0-flash".into()];
    config.circuit_breaker.failure_threshold = 2;
    let app = make_app(config).await;

    // Two failures to trip the breaker.
    for _ in 0..2 {
        let (s, _b) = json_body(app.clone().oneshot(authed(chat_req("groq/openai/gpt-oss-120b"))).await.unwrap()).await;
        assert_eq!(s, StatusCode::OK, "fallback should serve while circuit is closed");
    }
    let calls_before = broken_mock.seen.lock().unwrap().len();
    assert_eq!(calls_before, 2);

    // Circuit now open: groq must not be contacted again.
    let (status, body) = json_body(app.oneshot(authed(chat_req("groq/openai/gpt-oss-120b"))).await.unwrap()).await;
    assert_eq!(status, StatusCode::OK, "body: {}", body);
    assert_eq!(body["synthia"]["provider"], "gemini");
    let attempts = body["synthia"]["attempts"].as_array().unwrap();
    assert_eq!(attempts[0]["outcome"], "skipped_circuit_open");
    assert_eq!(broken_mock.seen.lock().unwrap().len(), calls_before, "open circuit must not be called");
}

/// Gemini's API key must travel in the x-goog-api-key header, never in the
/// URL query string (URLs land in reqwest error messages and logs).
#[tokio::test]
async fn gemini_key_stays_out_of_the_url() {
    let mut responses = HashMap::new();
    responses.insert("gemini-2.0-flash".to_string(), (200, json!({
        "candidates": [{"content": {"parts": [{"text": "hi"}]}}],
        "usageMetadata": {"promptTokenCount": 1, "candidatesTokenCount": 1, "totalTokenCount": 2}
    })));
    let (url, mock) = spawn_mock(responses).await;

    let mut config = base_config();
    config.providers.gemini = Some(GeminiConfig {
        enabled: true,
        api_key: Some("gemini-super-secret-key".into()),
        base_url: url,
        default_model: "gemini-2.0-flash".into(),
        available_models: vec!["gemini-2.0-flash".into()],
    });
    let app = make_app(config).await;

    let (status, _b) = json_body(app.oneshot(authed(chat_req("gemini/gemini-2.0-flash"))).await.unwrap()).await;
    assert_eq!(status, StatusCode::OK);

    let seen = mock.seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    let (uri, headers, _body) = &seen[0];
    assert!(!uri.contains("key="), "api key must not appear in the URL: {}", uri);
    assert_eq!(
        headers.get("x-goog-api-key").and_then(|v| v.to_str().ok()),
        Some("gemini-super-secret-key")
    );
}

/// Status/providers endpoints must never echo key material, and a transport
/// failure must not leak the provider key either.
#[tokio::test]
async fn secrets_never_appear_in_api_surfaces() {
    let (dead_url, _m) = spawn_mock(HashMap::new()).await;
    drop(dead_url);
    // A bound-then-dropped listener gives us a guaranteed-closed port.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let dead_addr = listener.local_addr().unwrap();
    drop(listener);

    let mut config = base_config();
    config.providers.groq = Some(groq_cfg(&format!("http://{}", dead_addr)));
    let app = make_app(config).await;

    for uri in ["/synthia/status", "/synthia/providers", "/synthia/spend"] {
        let req = Request::builder()
            .uri(uri)
            .header("authorization", format!("Bearer {}", GW_KEY))
            .body(Body::empty())
            .unwrap();
        let (status, body) = json_body(app.clone().oneshot(req).await.unwrap()).await;
        assert_eq!(status, StatusCode::OK);
        assert!(
            !body.to_string().contains("gsk_test_secret_key"),
            "{} leaked the groq key: {}",
            uri,
            body
        );
    }

    // Transport error from a dead upstream: the surfaced error must not
    // contain the key.
    let (status, body) = json_body(app.oneshot(authed(chat_req("groq/openai/gpt-oss-120b"))).await.unwrap()).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert!(!body.to_string().contains("gsk_test_secret_key"));
}

/// The budget middleware halts completion traffic when the daily budget is
/// exhausted, while status endpoints stay reachable.
#[tokio::test]
async fn budget_middleware_halts_completions_but_not_status() {
    let mut responses = HashMap::new();
    responses.insert("openai/gpt-oss-120b".to_string(), (200, ok_completion("openai/gpt-oss-120b")));
    let (url, _m) = spawn_mock(responses).await;

    let mut config = base_config();
    config.providers.groq = Some(groq_cfg(&url));
    config.circuit_breaker.daily_budget_usd = 0.0; // already exhausted
    let app = make_app(config).await;

    let (status, body) = json_body(app.clone().oneshot(authed(chat_req("groq/openai/gpt-oss-120b"))).await.unwrap()).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "body: {}", body);
    assert_eq!(body["error"]["code"], "budget_exceeded");

    let req = Request::builder()
        .uri("/synthia/status")
        .header("authorization", format!("Bearer {}", GW_KEY))
        .body(Body::empty())
        .unwrap();
    let (status, _b) = json_body(app.oneshot(req).await.unwrap()).await;
    assert_eq!(status, StatusCode::OK, "status must survive a budget halt");
}

/// Free Groq traffic must not push spend toward the budget breaker.
#[tokio::test]
async fn free_groq_calls_do_not_consume_budget() {
    let mut responses = HashMap::new();
    responses.insert("openai/gpt-oss-120b".to_string(), (200, ok_completion("openai/gpt-oss-120b")));
    let (url, _m) = spawn_mock(responses).await;

    let mut config = base_config();
    config.providers.groq = Some(groq_cfg(&url));
    // Tiny budget: any real cost would trip the breaker on the second call.
    config.circuit_breaker.daily_budget_usd = 0.000001;
    let app = make_app(config).await;

    for i in 0..3 {
        let (status, body) = json_body(app.clone().oneshot(authed(chat_req("groq/openai/gpt-oss-120b"))).await.unwrap()).await;
        assert_eq!(status, StatusCode::OK, "free call {} must stay unblocked: {}", i, body);
        assert_eq!(body["synthia"]["cost_usd"], 0.0);
    }

    let req = Request::builder()
        .uri("/synthia/spend")
        .header("authorization", format!("Bearer {}", GW_KEY))
        .body(Body::empty())
        .unwrap();
    let (_s, body) = json_body(app.oneshot(req).await.unwrap()).await;
    assert_eq!(body["today_usd"], 0.0);
}

/// A training-on-inputs provider refuses standard (confidential) traffic and
/// only serves explicitly non-confidential requests.
#[tokio::test]
async fn restricted_provider_requires_non_confidential_tag() {
    let mut responses = HashMap::new();
    responses.insert("z1-mini".to_string(), (200, ok_completion("z1-mini")));
    let (url, _m) = spawn_mock(responses).await;

    let mut config = base_config();
    config.providers.zai = Some(ZaiConfig {
        enabled: true,
        api_key: Some("zai-key".into()),
        base_url: url,
        default_model: "z1-mini".into(),
        available_models: vec!["z1-mini".into()],
    });
    let app = make_app(config).await;

    // Standard (default) traffic: refused.
    let (status, body) = json_body(app.clone().oneshot(authed(chat_req("zai/z1-mini"))).await.unwrap()).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "body: {}", body);
    assert_eq!(body["error"]["code"], "restricted_provider");

    // Explicitly tagged non-confidential: served.
    let mut req = chat_req("zai/z1-mini");
    req["metadata"] = json!({"data_class": "non-confidential"});
    let (status, body) = json_body(app.oneshot(authed(req)).await.unwrap()).await;
    assert_eq!(status, StatusCode::OK, "body: {}", body);
    assert_eq!(body["synthia"]["provider"], "zai");
    assert_eq!(body["synthia"]["data_class"], "non-confidential");
}

/// Restricted providers in the fallback chain are skipped for standard
/// traffic even when every other lane is down.
#[tokio::test]
async fn restricted_fallback_is_skipped_for_confidential_traffic() {
    let mut groq_responses = HashMap::new();
    groq_responses.insert("openai/gpt-oss-120b".to_string(), (500, json!({"error": "down"})));
    let (groq_url, _g) = spawn_mock(groq_responses).await;

    let mut zai_responses = HashMap::new();
    zai_responses.insert("z1-mini".to_string(), (200, ok_completion("z1-mini")));
    let (zai_url, zai_mock) = spawn_mock(zai_responses).await;

    let mut config = base_config();
    config.providers.groq = Some(groq_cfg(&groq_url));
    config.providers.zai = Some(ZaiConfig {
        enabled: true,
        api_key: Some("zai-key".into()),
        base_url: zai_url,
        default_model: "z1-mini".into(),
        available_models: vec!["z1-mini".into()],
    });
    config.routing.fallback_chain = vec!["zai/z1-mini".into()];
    config.circuit_breaker.failure_threshold = 100; // keep circuit closed for this test
    let app = make_app(config).await;

    let (status, body) = json_body(app.oneshot(authed(chat_req("groq/openai/gpt-oss-120b"))).await.unwrap()).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY, "must fail closed, not leak to zai: {}", body);
    assert_eq!(zai_mock.seen.lock().unwrap().len(), 0, "zai must never be called");
    let attempts = body["error"]["attempts"]["attempts"].as_array().unwrap();
    assert_eq!(attempts[1]["outcome"], "skipped_restricted");
}

/// Enabled-but-keyless provider is reported, never called without auth.
#[tokio::test]
async fn enabled_without_credentials_is_skipped_loudly() {
    let mut config = base_config();
    config.providers.groq = Some(GroqConfig {
        enabled: true,
        api_key: None,
        base_url: "http://127.0.0.1:1".into(),
        default_model: "openai/gpt-oss-120b".into(),
        available_models: vec![],
    });
    let app = make_app(config).await;

    let (status, body) = json_body(app.oneshot(authed(chat_req("groq/openai/gpt-oss-120b"))).await.unwrap()).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    let attempts = body["error"]["attempts"]["attempts"].as_array().unwrap();
    assert_eq!(attempts[0]["outcome"], "skipped_misconfigured");
}

/// Cloudflare is an inactive lane until explicitly enabled with credentials.
#[tokio::test]
async fn cloudflare_lane_is_dark_until_configured() {
    let config = base_config();
    let app = make_app(config).await;

    let (status, body) = json_body(app.oneshot(authed(chat_req("cloudflare/@cf/meta/llama-3.3-70b-instruct-fp8-fast"))).await.unwrap()).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    let attempts = body["error"]["attempts"]["attempts"].as_array().unwrap();
    assert_eq!(attempts[0]["outcome"], "skipped_not_configured");

    // Enabled but missing account id/token: misconfigured, never called.
    let mut config2 = base_config();
    config2.providers.cloudflare = Some(CloudflareConfig {
        enabled: true,
        api_key: None,
        account_id: None,
        default_model: "@cf/meta/llama-3.3-70b-instruct-fp8-fast".into(),
        available_models: vec![],
    });
    let app2 = make_app(config2).await;
    let (status, body) = json_body(app2.oneshot(authed(chat_req("cloudflare/@cf/meta/llama-3.3-70b-instruct-fp8-fast"))).await.unwrap()).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    let attempts = body["error"]["attempts"]["attempts"].as_array().unwrap();
    assert_eq!(attempts[0]["outcome"], "skipped_misconfigured");
}

/// Unauthenticated requests get 401 before anything else happens.
#[tokio::test]
async fn auth_is_enforced() {
    let app = make_app(base_config()).await;
    let req = Request::builder()
        .method("POST")
        .uri("/v1/chat/completions")
        .header("content-type", "application/json")
        .body(Body::from(chat_req("groq/openai/gpt-oss-120b").to_string()))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

/// Per-minute (transient) rate limits also fall back — they just classify
/// differently from the daily cap.
#[tokio::test]
async fn transient_429_also_falls_back_with_correct_classification() {
    let mut responses = HashMap::new();
    responses.insert(
        "openai/gpt-oss-120b".to_string(),
        (429, json!({"error": {"message": "Rate limit reached on tokens per minute (TPM): Limit 8000. Please try again in 5s.", "code": "rate_limit_exceeded"}})),
    );
    responses.insert("groq/compound".to_string(), (200, ok_completion("groq/compound")));
    let (url, _m) = spawn_mock(responses).await;

    let mut config = base_config();
    config.providers.groq = Some(groq_cfg(&url));
    config.routing.fallback_chain = vec!["groq/groq/compound".into()];
    let app = make_app(config).await;

    let (status, body) = json_body(app.oneshot(authed(chat_req("groq/openai/gpt-oss-120b"))).await.unwrap()).await;
    assert_eq!(status, StatusCode::OK);
    let attempts = body["synthia"]["attempts"].as_array().unwrap();
    assert_eq!(attempts[0]["failure"], "rate_limit_transient");
}

/// A model whose daily cap keeps failing opens only its own per-model
/// circuit — the healthy sibling model on the same provider keeps serving.
#[tokio::test]
async fn daily_cap_circuit_is_per_model_not_per_provider() {
    let mut responses = HashMap::new();
    responses.insert("openai/gpt-oss-120b".to_string(), (429, groq_tpd_body()));
    responses.insert("groq/compound".to_string(), (200, ok_completion("groq/compound")));
    let (url, mock) = spawn_mock(responses).await;

    let mut config = base_config();
    config.providers.groq = Some(groq_cfg(&url));
    config.routing.fallback_chain = vec!["groq/groq/compound".into()];
    config.circuit_breaker.failure_threshold = 1; // one failure opens
    let app = make_app(config).await;

    // First request: gpt-oss fails daily, compound serves, model circuit opens.
    let (status, _b) = json_body(app.clone().oneshot(authed(chat_req("groq/openai/gpt-oss-120b"))).await.unwrap()).await;
    assert_eq!(status, StatusCode::OK);

    // Second request: gpt-oss skipped (per-model circuit open) but compound
    // must still be called — the provider-wide circuit is closed.
    let (status, body) = json_body(app.oneshot(authed(chat_req("groq/openai/gpt-oss-120b"))).await.unwrap()).await;
    assert_eq!(status, StatusCode::OK, "body: {}", body);
    assert_eq!(body["synthia"]["model"], "groq/compound");
    let attempts = body["synthia"]["attempts"].as_array().unwrap();
    assert_eq!(attempts[0]["outcome"], "skipped_circuit_open");
    let compound_calls = mock
        .seen
        .lock()
        .unwrap()
        .iter()
        .filter(|(_, _, b)| b["model"] == "groq/compound")
        .count();
    assert_eq!(compound_calls, 2);
}

/// 401 is fail-closed: the provider rejected our credential, so the chain
/// stops and the error surfaces instead of silently serving elsewhere.
#[tokio::test]
async fn auth_401_stops_the_chain_fail_closed() {
    let mut responses = HashMap::new();
    responses.insert(
        "openai/gpt-oss-120b".to_string(),
        (401, json!({"error": {"message": "invalid api key"}})),
    );
    responses.insert("groq/compound".to_string(), (200, ok_completion("groq/compound")));
    let (url, mock) = spawn_mock(responses).await;

    let mut config = base_config();
    config.providers.groq = Some(groq_cfg(&url));
    config.routing.fallback_chain = vec!["groq/groq/compound".into()];
    let app = make_app(config).await;

    let (status, body) = json_body(app.oneshot(authed(chat_req("groq/openai/gpt-oss-120b"))).await.unwrap()).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY, "body: {}", body);
    assert_eq!(body["error"]["code"], "all_providers_failed");
    let attempts = body["error"]["attempts"]["attempts"].as_array().unwrap();
    assert_eq!(attempts.len(), 1, "chain must stop after the auth failure");
    assert_eq!(attempts[0]["failure"], "auth_error");
    assert_eq!(mock.seen.lock().unwrap().len(), 1, "fallback must not be called");
}

/// 403 behaves identically: fail-closed, one attempt, loud error.
#[tokio::test]
async fn auth_403_stops_the_chain_fail_closed() {
    let mut responses = HashMap::new();
    responses.insert(
        "openai/gpt-oss-120b".to_string(),
        (403, json!({"error": {"message": "forbidden"}})),
    );
    responses.insert("groq/compound".to_string(), (200, ok_completion("groq/compound")));
    let (url, mock) = spawn_mock(responses).await;

    let mut config = base_config();
    config.providers.groq = Some(groq_cfg(&url));
    config.routing.fallback_chain = vec!["groq/groq/compound".into()];
    let app = make_app(config).await;

    let (status, body) = json_body(app.oneshot(authed(chat_req("groq/openai/gpt-oss-120b"))).await.unwrap()).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    let attempts = body["error"]["attempts"]["attempts"].as_array().unwrap();
    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0]["failure"], "auth_error");
    assert_eq!(mock.seen.lock().unwrap().len(), 1);
}

/// An upstream error body full of multibyte UTF-8 past the 512-byte cut must
/// not panic truncation, and the fallback + receipt path must survive it.
#[tokio::test]
async fn multibyte_upstream_error_body_never_panics() {
    let mut big = "é".repeat(400); // 800 bytes of 2-byte chars
    big.push('😀'); // 4-byte char straddling any even cut
    big.push_str(&"漢字".repeat(200));
    let mut responses = HashMap::new();
    responses.insert(
        "openai/gpt-oss-120b".to_string(),
        (500, json!({"error": {"message": big}})),
    );
    responses.insert("groq/compound".to_string(), (200, ok_completion("groq/compound")));
    let (url, _m) = spawn_mock(responses).await;

    let mut config = base_config();
    config.providers.groq = Some(groq_cfg(&url));
    config.routing.fallback_chain = vec!["groq/groq/compound".into()];
    let app = make_app(config).await;

    let (status, body) = json_body(app.oneshot(authed(chat_req("groq/openai/gpt-oss-120b"))).await.unwrap()).await;
    assert_eq!(status, StatusCode::OK, "fallback must survive the big multibyte error: {}", body);
    let attempts = body["synthia"]["attempts"].as_array().unwrap();
    assert_eq!(attempts[0]["outcome"], "failed");
    assert_eq!(attempts[0]["failure"], "server_error");
    assert_eq!(attempts[1]["outcome"], "success");
    // The surfaced error JSON is valid UTF-8 with no replacement chars.
    assert!(!body.to_string().contains('\u{fffd}'));
}
