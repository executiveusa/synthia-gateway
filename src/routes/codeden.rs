// CodeDen handoff — forwards coding jobs to the CodeDen planMode pipeline.
//
// CodeDen lives on the tailnet (e.g. http://100.x.x.x:4318) and is not an LLM
// provider, so it gets its own route group instead of a Provider impl.
//
//   POST /codeden/code-task            { title?, prd|message, provider? }
//   GET  /codeden/code-task/:session_id  -> proxied session status
//
// Config (env, secrets via Infisical):
//   CODEDEN_BASE_URL   required to enable
//   CODEDEN_TOKEN      bearer token for the CodeDen API
//   CODEDEN_ENABLED    defaults to true when CODEDEN_BASE_URL is set

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/code-task", post(submit_code_task))
        .route("/code-task/:session_id", get(get_code_task))
}

#[derive(Debug, Deserialize)]
struct CodeTaskRequest {
    /// Short task title; derived from the brief when omitted.
    title: Option<String>,
    /// The product brief / requirements doc CodeDen plans from.
    prd: Option<String>,
    /// Accepted as an alias for prd so chat-style callers can post { message }.
    message: Option<String>,
    /// CodeDen provider to plan with; CodeDen default (claude) when omitted.
    provider: Option<String>,
}

fn codeden_config(state: &AppState) -> Option<&crate::config::CodeDenConfig> {
    state.config.codeden.as_ref().filter(|c| c.enabled)
}

fn not_configured() -> (StatusCode, Json<Value>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({ "error": "CodeDen not configured (CODEDEN_BASE_URL/CODEDEN_TOKEN)" })),
    )
}

async fn submit_code_task(
    State(state): State<AppState>,
    Json(body): Json<CodeTaskRequest>,
) -> impl IntoResponse {
    let Some(cfg) = codeden_config(&state) else {
        return not_configured().into_response();
    };

    let prd = body.prd.or(body.message).unwrap_or_default();
    if prd.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "prd (or message) is required" })),
        )
            .into_response();
    }
    let title = body
        .title
        .unwrap_or_else(|| prd.chars().take(60).collect::<String>());

    let url = format!("{}/api/sessions", cfg.base_url.trim_end_matches('/'));
    let mut req = state.http.post(&url).json(&json!({
        "title": title,
        "prd": prd,
        "provider": body.provider.unwrap_or_else(|| "claude".into()),
    }));
    if let Some(token) = &cfg.token {
        req = req.bearer_auth(token);
    }

    match req.send().await {
        Ok(res) if res.status().is_success() => {
            let session = res.json::<Value>().await.unwrap_or_else(|_| json!({}));
            (
                StatusCode::ACCEPTED,
                Json(json!({ "forwarded": true, "codeden": session })),
            )
                .into_response()
        }
        Ok(res) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "error": "CodeDen handoff failed", "status": res.status().as_u16() })),
        )
            .into_response(),
        Err(err) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "error": format!("CodeDen unreachable: {err}") })),
        )
            .into_response(),
    }
}

async fn get_code_task(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    let Some(cfg) = codeden_config(&state) else {
        return not_configured().into_response();
    };

    let url = format!(
        "{}/api/sessions/{}",
        cfg.base_url.trim_end_matches('/'),
        session_id
    );
    let mut req = state.http.get(&url);
    if let Some(token) = &cfg.token {
        req = req.bearer_auth(token);
    }

    match req.send().await {
        Ok(res) => {
            let status = StatusCode::from_u16(res.status().as_u16())
                .unwrap_or(StatusCode::BAD_GATEWAY);
            let body = res.json::<Value>().await.unwrap_or_else(|_| json!({}));
            (status, Json(body)).into_response()
        }
        Err(err) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "error": format!("CodeDen unreachable: {err}") })),
        )
            .into_response(),
    }
}
