use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
    Json,
};
use serde_json::json;
use crate::state::AppState;

pub async fn check_budget(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    let path = req.uri().path();
    // The budget gate guards completion endpoints only. Health, status and
    // admin stay reachable so operators can still inspect a halted gateway.
    if path.starts_with("/health") || path.starts_with("/synthia") || path.starts_with("/admin") {
        return Ok(next.run(req).await);
    }

    if state.is_budget_exceeded().await {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({
                "error": {
                    "code": "budget_exceeded",
                    "message": "Daily API budget exceeded. Gateway halted. Contact admin.",
                    "type": "gateway_error"
                }
            }))
        ));
    }
    Ok(next.run(req).await)
}
