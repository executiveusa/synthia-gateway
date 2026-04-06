// z.ai — speaks OpenAI API format

use anyhow::Result;
use serde_json::Value;
use crate::config::ZaiConfig;
use crate::config::OpenAIConfig;
use crate::providers::openai::OpenAIProvider;

pub struct ZaiProvider {
    inner: OpenAIProvider,
}

impl ZaiProvider {
    pub fn new(config: ZaiConfig) -> Self {
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
impl super::Provider for ZaiProvider {
    fn name(&self) -> &str { "zai" }

    async fn chat(&self, request: Value) -> Result<Value> {
        self.inner.chat(request).await
    }

    async fn chat_stream(&self, request: Value) -> Result<Vec<u8>> {
        self.inner.chat_stream(request).await
    }

    async fn models(&self) -> Result<Vec<super::ProviderModel>> {
        let mut models = self.inner.models().await?;
        for m in &mut models {
            m.provider = "zai".into();
        }
        Ok(models)
    }
}
