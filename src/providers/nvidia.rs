// NVIDIA NIM — speaks OpenAI API format at integrate.api.nvidia.com

use anyhow::Result;
use serde_json::Value;
use crate::config::NvidiaConfig;
use crate::config::OpenAIConfig;
use crate::providers::openai::OpenAIProvider;

pub struct NvidiaProvider {
    inner: OpenAIProvider,
}

impl NvidiaProvider {
    pub fn new(config: NvidiaConfig) -> Self {
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
impl super::Provider for NvidiaProvider {
    fn name(&self) -> &str { "nvidia" }

    async fn chat(&self, request: Value) -> Result<Value> {
        self.inner.chat(request).await
    }

    async fn chat_stream(&self, request: Value) -> Result<Vec<u8>> {
        self.inner.chat_stream(request).await
    }

    async fn models(&self) -> Result<Vec<super::ProviderModel>> {
        let mut models = self.inner.models().await?;
        for m in &mut models {
            m.provider = "nvidia".into();
        }
        Ok(models)
    }
}
