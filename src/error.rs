use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GatewayError {
    #[error("Unauthorized: invalid or missing gateway API key")]
    Unauthorized,

    #[error("Provider '{0}' is not configured or disabled")]
    ProviderNotFound(String),

    #[error("Provider '{0}' circuit breaker is open — too many failures")]
    CircuitOpen(String),

    #[error("Daily budget exceeded — gateway halted")]
    BudgetExceeded,

    #[error("Provider request failed: {0}")]
    ProviderError(String),

    #[error("Model '{0}' not recognized")]
    UnknownModel(String),

    #[error("Request body is invalid: {0}")]
    BadRequest(String),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("HTTP client error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for GatewayError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            GatewayError::Unauthorized =>
                (StatusCode::UNAUTHORIZED, "unauthorized", self.to_string()),
            GatewayError::ProviderNotFound(_) =>
                (StatusCode::BAD_REQUEST, "provider_not_found", self.to_string()),
            GatewayError::CircuitOpen(_) =>
                (StatusCode::SERVICE_UNAVAILABLE, "circuit_open", self.to_string()),
            GatewayError::BudgetExceeded =>
                (StatusCode::TOO_MANY_REQUESTS, "budget_exceeded", self.to_string()),
            GatewayError::ProviderError(_) =>
                (StatusCode::BAD_GATEWAY, "provider_error", self.to_string()),
            GatewayError::UnknownModel(_) =>
                (StatusCode::BAD_REQUEST, "unknown_model", self.to_string()),
            GatewayError::BadRequest(_) =>
                (StatusCode::BAD_REQUEST, "bad_request", self.to_string()),
            _ =>
                (StatusCode::INTERNAL_SERVER_ERROR, "internal_error", self.to_string()),
        };

        let body = Json(json!({
            "error": {
                "code": code,
                "message": message,
                "type": "gateway_error"
            }
        }));

        (status, body).into_response()
    }
}

pub type GatewayResult<T> = Result<T, GatewayError>;
