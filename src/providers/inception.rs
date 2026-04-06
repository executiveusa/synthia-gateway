// Mercury 2 by Inception Labs
// Stub until API is publicly available — set INCEPTION_ENABLED=true when live

use anyhow::Result;
use serde_json::Value;
use crate::config::InceptionConfig;

pub struct InceptionProvider {
    config: InceptionConfig,
    client: reqwest::Client,
}

impl InceptionProvider {
    pub fn new(config: InceptionConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait::async_trait]
impl super::Provider for InceptionProvider {
    fn name(&self) -> &str { "inception" }

    async fn chat(&self, request: Value) -> Result<Value> {
        if !self.config.enabled {
            return Err(anyhow::anyhow!(
                "Inception/Mercury 2 is not enabled. Set INCEPTION_ENABLED=true when API keys are available."
            ));
        }
        let url = format!("{}/chat/completions", self.config.base_url);
        let response = self
            .client
            .post(&url)
            .bearer_auth(self.config.api_key.as_deref().unwrap_or(""))
            .json(&request)
            .send()
            .await?
            .json::<Value>()
            .await?;
        Ok(response)
    }

    async fn chat_stream(&self, _request: Value) -> Result<Vec<u8>> {
        Err(anyhow::anyhow!("Mercury 2 streaming not yet available"))
    }

    async fn models(&self) -> Result<Vec<super::ProviderModel>> {
        Ok(self
            .config
            .available_models
            .iter()
            .map(|m| super::ProviderModel {
                id: m.clone(),
                provider: "inception".into(),
                alias: None,
            })
            .collect())
    }
}
