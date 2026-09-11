// Cloudflare Workers AI — OpenAI-compatible endpoint at
// https://api.cloudflare.com/client/v4/accounts/{ACCOUNT_ID}/ai/v1
//
// This is an INACTIVE lane by design: the config only materializes when
// CLOUDFLARE_ENABLED=true, and the adapter fails closed unless both the
// account id and API token are present.

use anyhow::Result;
use serde_json::Value;
use crate::config::CloudflareConfig;
use crate::config::OpenAIConfig;
use crate::providers::openai::OpenAIProvider;

pub struct CloudflareProvider {
    inner: OpenAIProvider,
}

impl CloudflareProvider {
    pub fn new(config: CloudflareConfig) -> Result<Self> {
        let account_id = config
            .account_id
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("CLOUDFLARE_ACCOUNT_ID is required for the cloudflare provider"))?
            .to_string();
        let base_url = std::env::var("CLOUDFLARE_BASE_URL").unwrap_or_else(|_| {
            format!(
                "https://api.cloudflare.com/client/v4/accounts/{}/ai",
                account_id
            )
        });
        let openai_cfg = OpenAIConfig {
            enabled: config.enabled,
            api_key: config.api_key,
            base_url,
            default_model: config.default_model,
            available_models: config.available_models,
        };
        Ok(Self {
            inner: OpenAIProvider::new(openai_cfg),
        })
    }
}

#[async_trait::async_trait]
impl super::Provider for CloudflareProvider {
    fn name(&self) -> &str { "cloudflare" }

    async fn chat(&self, request: Value) -> Result<Value> {
        self.inner.chat(request).await
    }

    async fn chat_stream(&self, request: Value) -> Result<Vec<u8>> {
        self.inner.chat_stream(request).await
    }

    async fn models(&self) -> Result<Vec<super::ProviderModel>> {
        let mut models = self.inner.models().await?;
        for m in &mut models {
            m.provider = "cloudflare".into();
        }
        Ok(models)
    }
}
