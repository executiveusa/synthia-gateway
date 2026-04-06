pub mod router;
pub mod anthropic;
pub mod openai;
pub mod gemini;
pub mod nvidia;
pub mod inception;
pub mod zai;
pub mod ollama;
pub mod openrouter;

use anyhow::Result;

#[async_trait::async_trait]
pub trait Provider: Send + Sync {
    fn name(&self) -> &str;
    async fn chat(&self, request: serde_json::Value) -> Result<serde_json::Value>;
    async fn chat_stream(&self, request: serde_json::Value) -> Result<Vec<u8>>;
    async fn models(&self) -> Result<Vec<ProviderModel>>;
}

#[derive(Debug, serde::Serialize)]
pub struct ProviderModel {
    pub id: String,
    pub provider: String,
    pub alias: Option<String>,
}
