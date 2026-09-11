// Groq — OpenAI-compatible at https://api.groq.com/openai/v1
// StarNet's $0 default brain: openai/gpt-oss-120b with groq/compound as the
// pressure valve when the daily token cap (TPD) is hit.

use anyhow::Result;
use serde_json::Value;
use crate::config::GroqConfig;
use crate::config::OpenAIConfig;
use crate::providers::openai::OpenAIProvider;

pub struct GroqProvider {
    inner: OpenAIProvider,
}

impl GroqProvider {
    pub fn new(config: GroqConfig) -> Self {
        let openai_cfg = OpenAIConfig {
            enabled: config.enabled,
            api_key: config.api_key,
            base_url: config.base_url,
            default_model: config.default_model,
            available_models: config.available_models,
        };
        Self {
            inner: OpenAIProvider::new(openai_cfg),
        }
    }
}

#[async_trait::async_trait]
impl super::Provider for GroqProvider {
    fn name(&self) -> &str { "groq" }

    async fn chat(&self, request: Value) -> Result<Value> {
        self.inner.chat(request).await
    }

    async fn chat_stream(&self, request: Value) -> Result<Vec<u8>> {
        self.inner.chat_stream(request).await
    }

    async fn models(&self) -> Result<Vec<super::ProviderModel>> {
        let mut models = self.inner.models().await?;
        for m in &mut models {
            m.provider = "groq".into();
        }
        Ok(models)
    }
}
