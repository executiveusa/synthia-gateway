// OpenRouter — speaks OpenAI format, requires HTTP-Referer header

use anyhow::Result;
use serde_json::Value;
use crate::config::OpenRouterConfig;
use crate::config::OpenAIConfig;
use crate::providers::openai::OpenAIProvider;

pub struct OpenRouterProvider {
    inner: OpenAIProvider,
}

impl OpenRouterProvider {
    pub fn new(config: OpenRouterConfig) -> Self {
        let openai_cfg = OpenAIConfig {
            enabled: config.enabled,
            api_key: config.api_key,
            base_url: config.base_url,
            default_model: config.default_model,
            available_models: config.available_models,
        };
        Self {
            inner: OpenAIProvider::new(openai_cfg).with_headers(vec![
                ("HTTP-Referer".into(), "https://synthia.gateway".into()),
                ("X-Title".into(), "SYNTHIA Gateway".into()),
            ]),
        }
    }
}

#[async_trait::async_trait]
impl super::Provider for OpenRouterProvider {
    fn name(&self) -> &str { "openrouter" }

    async fn chat(&self, request: Value) -> Result<Value> {
        self.inner.chat(request).await
    }

    async fn chat_stream(&self, request: Value) -> Result<Vec<u8>> {
        self.inner.chat_stream(request).await
    }

    async fn models(&self) -> Result<Vec<super::ProviderModel>> {
        let mut models = self.inner.models().await?;
        for m in &mut models {
            m.provider = "openrouter".into();
        }
        Ok(models)
    }
}
