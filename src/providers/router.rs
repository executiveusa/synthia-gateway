use crate::config::GatewayConfig;
use anyhow::{anyhow, Result};

pub struct RouteTarget {
    pub provider: String,
    pub model: String,
}

pub struct Router {
    config: GatewayConfig,
}

impl Router {
    pub fn new(config: GatewayConfig) -> Self {
        Self { config }
    }

    /// Resolve a model string to a (provider, model) pair.
    ///
    /// Resolution order:
    /// 1. Alias lookup (e.g. "fast" → "anthropic/claude-haiku-...")
    /// 2. Explicit prefix (e.g. "anthropic/claude-sonnet-4-20250514")
    /// 3. Default provider with the model name as-is
    pub fn resolve(&self, model: &str) -> Result<RouteTarget> {
        // 1. Check aliases
        if let Some(aliased) = self.config.routing.aliases.get(model) {
            return self.split_provider_model(aliased);
        }

        // 2. Explicit provider/model prefix
        if model.contains('/') {
            return self.split_provider_model(model);
        }

        // 3. Default provider
        let provider = self.config.routing.default_provider.clone();
        Ok(RouteTarget {
            provider,
            model: model.to_string(),
        })
    }

    fn split_provider_model(&self, s: &str) -> Result<RouteTarget> {
        let Some(slash) = s.find('/') else {
            return Err(anyhow!("Expected 'provider/model' format, got: {}", s));
        };
        let provider = s[..slash].to_string();
        let model = s[slash + 1..].to_string();
        Ok(RouteTarget { provider, model })
    }
}
