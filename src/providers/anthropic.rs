// Anthropic provider — handles x-api-key auth and translates
// between OpenAI format and Anthropic Messages API

use anyhow::Result;
use reqwest::Client;
use serde_json::{json, Value};
use tracing::{debug, error};

use crate::config::AnthropicConfig;

pub struct AnthropicProvider {
    pub config: AnthropicConfig,
    client: Client,
}

impl AnthropicProvider {
    pub fn new(config: AnthropicConfig) -> Self {
        Self {
            config,
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .expect("Failed to build HTTP client"),
        }
    }

    fn to_anthropic_format(&self, request: &Value) -> Value {
        let model = request["model"]
            .as_str()
            .unwrap_or(&self.config.default_model)
            .to_string();

        let model = match model.as_str() {
            "claude-sonnet-4-5" | "claude-sonnet" => "claude-sonnet-4-20250514",
            "claude-opus-4-5" | "claude-opus" => "claude-opus-4-20250514",
            "claude-haiku-4-5" | "claude-haiku" => "claude-haiku-4-5-20251001",
            other => other,
        }
        .to_string();

        let messages = request["messages"].clone();
        let max_tokens = request["max_tokens"].as_i64().unwrap_or(4096);
        let temperature = request["temperature"].clone();
        let stream = request["stream"].as_bool().unwrap_or(false);

        let mut body = json!({
            "model": model,
            "messages": messages,
            "max_tokens": max_tokens,
            "stream": stream
        });

        if !temperature.is_null() {
            body["temperature"] = temperature;
        }

        if let Some(messages) = request["messages"].as_array() {
            let system = messages
                .iter()
                .find(|m| m["role"] == "system")
                .and_then(|m| m["content"].as_str())
                .map(String::from);

            if let Some(sys) = system {
                body["system"] = json!(sys);
                let filtered: Vec<&Value> =
                    messages.iter().filter(|m| m["role"] != "system").collect();
                body["messages"] = json!(filtered);
            }
        }

        body
    }

    fn to_openai_format(&self, response: Value, model: &str) -> Value {
        let content = response["content"][0]["text"].as_str().unwrap_or("");
        let input_tokens = response["usage"]["input_tokens"].as_i64().unwrap_or(0);
        let output_tokens = response["usage"]["output_tokens"].as_i64().unwrap_or(0);

        json!({
            "id": response["id"],
            "object": "chat.completion",
            "model": model,
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": content
                },
                "finish_reason": response["stop_reason"]
            }],
            "usage": {
                "prompt_tokens": input_tokens,
                "completion_tokens": output_tokens,
                "total_tokens": input_tokens + output_tokens
            }
        })
    }
}

#[async_trait::async_trait]
impl super::Provider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    async fn chat(&self, request: Value) -> Result<Value> {
        let model = request["model"]
            .as_str()
            .unwrap_or(&self.config.default_model)
            .to_string();

        let anthropic_request = self.to_anthropic_format(&request);
        let url = format!("{}/v1/messages", self.config.base_url);

        let mut req = self
            .client
            .post(&url)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&anthropic_request);

        if let Some(key) = &self.config.api_key {
            req = req.header("x-api-key", key);
        }

        let response = req.send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            error!("Anthropic error {}: {}", status, body);
            return Err(anyhow::anyhow!("Anthropic error {}: {}", status, body));
        }

        let anthropic_response: Value = response.json().await?;
        debug!(
            "Anthropic: {} tokens used",
            anthropic_response["usage"]["input_tokens"].as_i64().unwrap_or(0)
                + anthropic_response["usage"]["output_tokens"].as_i64().unwrap_or(0)
        );

        Ok(self.to_openai_format(anthropic_response, &model))
    }

    async fn chat_stream(&self, request: Value) -> Result<Vec<u8>> {
        let mut req_body = self.to_anthropic_format(&request);
        req_body["stream"] = json!(true);

        let url = format!("{}/v1/messages", self.config.base_url);

        let mut req = self
            .client
            .post(&url)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&req_body);

        if let Some(key) = &self.config.api_key {
            req = req.header("x-api-key", key);
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
                provider: "anthropic".into(),
                alias: None,
            })
            .collect())
    }
}
