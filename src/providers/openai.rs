// OpenAI provider — also used as the base for NVIDIA NIM, z.ai, and OpenRouter
// since they all speak the OpenAI API format

use anyhow::Result;
use reqwest::Client;
use serde_json::Value;
use tracing::{debug, error};

use crate::config::OpenAIConfig;

pub struct OpenAIProvider {
    config: OpenAIConfig,
    client: Client,
    /// Optional extra headers to send (e.g. HTTP-Referer for OpenRouter)
    extra_headers: Vec<(String, String)>,
}

impl OpenAIProvider {
    pub fn new(config: OpenAIConfig) -> Self {
        Self {
            config,
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .expect("Failed to build HTTP client"),
            extra_headers: vec![],
        }
    }

    pub fn with_headers(mut self, headers: Vec<(String, String)>) -> Self {
        self.extra_headers = headers;
        self
    }

    fn auth_header(&self) -> Option<String> {
        self.config.api_key.as_ref().map(|k| format!("Bearer {}", k))
    }
}

#[async_trait::async_trait]
impl super::Provider for OpenAIProvider {
    fn name(&self) -> &str {
        "openai"
    }

    async fn chat(&self, request: Value) -> Result<Value> {
        let url = format!("{}/v1/chat/completions", self.config.base_url);
        let mut req = self.client.post(&url).json(&request);

        if let Some(auth) = self.auth_header() {
            req = req.header("Authorization", auth);
        }
        for (k, v) in &self.extra_headers {
            req = req.header(k.as_str(), v.as_str());
        }

        let response = req.send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            error!("OpenAI error {}: {}", status, body);
            return Err(anyhow::anyhow!("OpenAI error {}: {}", status, body));
        }

        let json: Value = response.json().await?;
        debug!(
            "OpenAI response: {} tokens used",
            json["usage"]["total_tokens"].as_i64().unwrap_or(0)
        );

        Ok(json)
    }

    async fn chat_stream(&self, request: Value) -> Result<Vec<u8>> {
        let mut req_body = request.clone();
        req_body["stream"] = serde_json::json!(true);

        let url = format!("{}/v1/chat/completions", self.config.base_url);
        let mut req = self.client.post(&url).json(&req_body);

        if let Some(auth) = self.auth_header() {
            req = req.header("Authorization", auth);
        }
        for (k, v) in &self.extra_headers {
            req = req.header(k.as_str(), v.as_str());
        }

        let response = req.send().await?;
        let bytes = response.bytes().await?;
        Ok(bytes.to_vec())
    }

    async fn models(&self) -> Result<Vec<super::ProviderModel>> {
        Ok(self
            .config
            .available_models
            .iter()
            .map(|m| super::ProviderModel {
                id: m.clone(),
                provider: "openai".into(),
                alias: None,
            })
            .collect())
    }
}
