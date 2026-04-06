// Google Gemini — translates OpenAI format to Gemini generateContent API

use anyhow::Result;
use reqwest::Client;
use serde_json::{json, Value};
use tracing::error;

use crate::config::GeminiConfig;

pub struct GeminiProvider {
    config: GeminiConfig,
    client: Client,
}

impl GeminiProvider {
    pub fn new(config: GeminiConfig) -> Self {
        Self {
            config,
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .expect("Failed to build HTTP client"),
        }
    }

    fn to_gemini_format(&self, request: &Value) -> Value {
        let messages = request["messages"].as_array().cloned().unwrap_or_default();
        let contents: Vec<Value> = messages
            .iter()
            .filter(|m| m["role"] != "system")
            .map(|m| {
                let role = if m["role"] == "assistant" { "model" } else { "user" };
                json!({
                    "role": role,
                    "parts": [{"text": m["content"]}]
                })
            })
            .collect();

        let mut body = json!({ "contents": contents });

        if let Some(sys) = messages
            .iter()
            .find(|m| m["role"] == "system")
            .and_then(|m| m["content"].as_str())
        {
            body["systemInstruction"] = json!({
                "parts": [{"text": sys}]
            });
        }

        if let Some(max) = request["max_tokens"].as_i64() {
            body["generationConfig"] = json!({ "maxOutputTokens": max });
        }

        body
    }

    fn to_openai_format(&self, response: Value, model: &str) -> Value {
        let text = response["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .unwrap_or("");

        json!({
            "id": format!("gemini-{}", uuid::Uuid::new_v4()),
            "object": "chat.completion",
            "model": model,
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": text},
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": response["usageMetadata"]["promptTokenCount"].as_i64().unwrap_or(0),
                "completion_tokens": response["usageMetadata"]["candidatesTokenCount"].as_i64().unwrap_or(0),
                "total_tokens": response["usageMetadata"]["totalTokenCount"].as_i64().unwrap_or(0)
            }
        })
    }
}

#[async_trait::async_trait]
impl super::Provider for GeminiProvider {
    fn name(&self) -> &str {
        "gemini"
    }

    async fn chat(&self, request: Value) -> Result<Value> {
        let model = request["model"]
            .as_str()
            .unwrap_or(&self.config.default_model)
            .to_string();

        let url = format!(
            "{}/v1beta/models/{}:generateContent?key={}",
            self.config.base_url,
            model,
            self.config.api_key.as_deref().unwrap_or("")
        );

        let gemini_request = self.to_gemini_format(&request);
        let response = self
            .client
            .post(&url)
            .json(&gemini_request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            error!("Gemini error {}: {}", status, body);
            return Err(anyhow::anyhow!("Gemini error {}: {}", status, body));
        }

        let gemini_response: Value = response.json().await?;
        Ok(self.to_openai_format(gemini_response, &model))
    }

    async fn chat_stream(&self, request: Value) -> Result<Vec<u8>> {
        let model = request["model"]
            .as_str()
            .unwrap_or(&self.config.default_model)
            .to_string();
        let url = format!(
            "{}/v1beta/models/{}:streamGenerateContent?key={}&alt=sse",
            self.config.base_url,
            model,
            self.config.api_key.as_deref().unwrap_or("")
        );
        let gemini_request = self.to_gemini_format(&request);
        let response = self
            .client
            .post(&url)
            .json(&gemini_request)
            .send()
            .await?;
        Ok(response.bytes().await?.to_vec())
    }

    async fn models(&self) -> Result<Vec<super::ProviderModel>> {
        Ok(self
            .config
            .available_models
            .iter()
            .map(|m| super::ProviderModel {
                id: m.clone(),
                provider: "gemini".into(),
                alias: None,
            })
            .collect())
    }
}
