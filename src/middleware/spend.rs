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
