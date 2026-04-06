use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};
use crate::state::AppState;

pub async fn verify_gateway_key(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    // Skip auth on health endpoints
    if req.uri().path() == "/health" || req.uri().path().starts_with("/health/") {
        return Ok(next.run(req).await);
    }

    let key = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .or_else(|| {
            req.headers()
                .get("x-api-key")
                .and_then(|v| v.to_str().ok())
        });

    let Some(key) = key else {
        return Err(StatusCode::UNAUTHORIZED);
    };

    let valid = key == state.config.auth.gateway_key
        || state.config.auth.allowed_keys.contains(&key.to_string());

    if !valid {
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(next.run(req).await)
}

pub fn extract_agent_id(req: &Request<Body>) -> Option<String> {
    req.headers()
        .get("x-agent-id")
        .and_then(|v| v.to_str().ok())
        .map(String::from)
}
