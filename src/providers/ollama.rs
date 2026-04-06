// Ollama — speaks OpenAI format at /api/chat (no API key required)

use anyhow::Result;
use reqwest::Client;
use serde_json::Value;
use tracing::error;

use crate::config::OllamaConfig;

pub struct OllamaProvider {
    config: OllamaConfig,
    client: Client,
}

impl OllamaProvider {
    pub fn new(config: OllamaConfig) -> Self {
        Self {
            config,
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .expect("Failed to build HTTP client"),
        }
    }
}

#[async_trait::async_trait]
impl super::Provider for OllamaProvider {
    fn name(&self) -> &str { "ollama" }

    async fn chat(&self, request: Value) -> Result<Value> {
        // Ollama's OpenAI-compatible endpoint
        let url = format!("{}/v1/chat/completions", self.config.base_url);
        let response = self.client.post(&url).json(&request).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            error!("Ollama error {}: {}", status, body);
            return Err(anyhow::anyhow!("Ollama error {}: {}", status, body));
        }

        Ok(response.json::<Value>().await?)
    }

    async fn chat_stream(&self, request: Value) -> Result<Vec<u8>> {
        let mut req_body = request.clone();
        req_body["stream"] = serde_json::json!(true);
        let url = format!("{}/v1/chat/completions", self.config.base_url);
        let response = self.client.post(&url).json(&req_body).send().await?;
        Ok(response.bytes().await?.to_vec())
    }

    async fn models(&self) -> Result<Vec<super::ProviderModel>> {
        Ok(self
            .config
            .available_models
            .iter()
            .map(|m| super::ProviderModel {
                id: m.clone(),
                provider: "ollama".into(),
                alias: None,
            })
            .collect())
    }
}
