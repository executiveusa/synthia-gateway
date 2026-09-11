use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GatewayConfig {
    pub auth: AuthConfig,
    pub routing: RoutingConfig,
    pub circuit_breaker: CircuitBreakerConfig,
    pub safety: SafetyConfig,
    pub providers: ProvidersConfig,
}

/// Data-safety routing rules.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SafetyConfig {
    /// Providers whose free tiers may train on request data. They are never
    /// used unless the caller explicitly tags the request as non-confidential.
    pub trains_on_inputs_providers: Vec<String>,
}

impl Default for SafetyConfig {
    fn default() -> Self {
        Self {
            trains_on_inputs_providers: vec!["zai".to_string()],
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuthConfig {
    pub gateway_key: String,
    #[serde(default)]
    pub allowed_keys: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RoutingConfig {
    pub default_provider: String,
    #[serde(default)]
    pub fallback_chain: Vec<String>,
    #[serde(default)]
    pub aliases: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CircuitBreakerConfig {
    pub daily_budget_usd: f64,
    pub failure_threshold: u32,
    pub reset_seconds: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ProvidersConfig {
    pub anthropic: Option<AnthropicConfig>,
    pub openai: Option<OpenAIConfig>,
    pub gemini: Option<GeminiConfig>,
    pub nvidia: Option<NvidiaConfig>,
    pub inception: Option<InceptionConfig>,
    pub zai: Option<ZaiConfig>,
    pub ollama: Option<OllamaConfig>,
    pub openrouter: Option<OpenRouterConfig>,
    pub groq: Option<GroqConfig>,
    pub cloudflare: Option<CloudflareConfig>,
    pub byokey: Option<ByoKeyConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AnthropicConfig {
    pub enabled: bool,
    pub api_key: Option<String>,
    #[serde(default = "default_anthropic_url")]
    pub base_url: String,
    #[serde(default = "default_anthropic_model")]
    pub default_model: String,
    #[serde(default = "default_anthropic_models")]
    pub available_models: Vec<String>,
}

fn default_anthropic_url() -> String {
    "https://api.anthropic.com".into()
}
fn default_anthropic_model() -> String {
    "claude-sonnet-4-20250514".into()
}
fn default_anthropic_models() -> Vec<String> {
    vec![
        "claude-opus-4-20250514".into(),
        "claude-sonnet-4-20250514".into(),
        "claude-haiku-4-5-20251001".into(),
    ]
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OpenAIConfig {
    pub enabled: bool,
    pub api_key: Option<String>,
    #[serde(default = "default_openai_url")]
    pub base_url: String,
    #[serde(default = "default_openai_model")]
    pub default_model: String,
    #[serde(default = "default_openai_models")]
    pub available_models: Vec<String>,
}

fn default_openai_url() -> String {
    "https://api.openai.com".into()
}
fn default_openai_model() -> String {
    "gpt-4o".into()
}
fn default_openai_models() -> Vec<String> {
    vec!["gpt-4o".into(), "gpt-4o-mini".into(), "o1".into()]
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GeminiConfig {
    pub enabled: bool,
    pub api_key: Option<String>,
    #[serde(default = "default_gemini_url")]
    pub base_url: String,
    #[serde(default = "default_gemini_model")]
    pub default_model: String,
    #[serde(default = "default_gemini_models")]
    pub available_models: Vec<String>,
}

fn default_gemini_url() -> String {
    "https://generativelanguage.googleapis.com".into()
}
fn default_gemini_model() -> String {
    "gemini-2.0-flash".into()
}
fn default_gemini_models() -> Vec<String> {
    vec![
        "gemini-2.0-flash".into(),
        "gemini-2.0-flash-lite".into(),
        "gemini-1.5-pro".into(),
    ]
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NvidiaConfig {
    pub enabled: bool,
    pub api_key: Option<String>,
    #[serde(default = "default_nvidia_url")]
    pub base_url: String,
    #[serde(default = "default_nvidia_model")]
    pub default_model: String,
    #[serde(default = "default_nvidia_models")]
    pub available_models: Vec<String>,
}

fn default_nvidia_url() -> String {
    "https://integrate.api.nvidia.com".into()
}
fn default_nvidia_model() -> String {
    "meta/llama-3.3-70b-instruct".into()
}
fn default_nvidia_models() -> Vec<String> {
    vec![
        "meta/llama-3.3-70b-instruct".into(),
        "nvidia/llama-3.1-nemotron-ultra-253b-v1".into(),
    ]
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InceptionConfig {
    pub enabled: bool,
    pub api_key: Option<String>,
    #[serde(default = "default_inception_url")]
    pub base_url: String,
    #[serde(default = "default_inception_model")]
    pub default_model: String,
    #[serde(default = "default_inception_models")]
    pub available_models: Vec<String>,
}

fn default_inception_url() -> String {
    "https://api.inception.ai".into()
}
fn default_inception_model() -> String {
    "mercury-2".into()
}
fn default_inception_models() -> Vec<String> {
    vec!["mercury-2".into()]
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ZaiConfig {
    pub enabled: bool,
    pub api_key: Option<String>,
    #[serde(default = "default_zai_url")]
    pub base_url: String,
    #[serde(default = "default_zai_model")]
    pub default_model: String,
    #[serde(default = "default_zai_models")]
    pub available_models: Vec<String>,
}

fn default_zai_url() -> String {
    "https://api.z.ai".into()
}
fn default_zai_model() -> String {
    "z1-mini".into()
}
fn default_zai_models() -> Vec<String> {
    vec!["z1-mini".into(), "z1".into()]
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OllamaConfig {
    pub enabled: bool,
    #[serde(default = "default_ollama_url")]
    pub base_url: String,
    #[serde(default = "default_ollama_model")]
    pub default_model: String,
    #[serde(default = "default_ollama_models")]
    pub available_models: Vec<String>,
}

fn default_ollama_url() -> String {
    "http://localhost:11434".into()
}
fn default_ollama_model() -> String {
    "llama3.2".into()
}
fn default_ollama_models() -> Vec<String> {
    vec!["llama3.2".into(), "mistral".into(), "phi4".into()]
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OpenRouterConfig {
    pub enabled: bool,
    pub api_key: Option<String>,
    #[serde(default = "default_openrouter_url")]
    pub base_url: String,
    #[serde(default = "default_openrouter_model")]
    pub default_model: String,
    #[serde(default)]
    pub available_models: Vec<String>,
}

fn default_openrouter_url() -> String {
    "https://openrouter.ai/api".into()
}
fn default_openrouter_model() -> String {
    "openai/gpt-4o".into()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GroqConfig {
    pub enabled: bool,
    pub api_key: Option<String>,
    #[serde(default = "default_groq_url")]
    pub base_url: String,
    #[serde(default = "default_groq_model")]
    pub default_model: String,
    #[serde(default = "default_groq_models")]
    pub available_models: Vec<String>,
}

fn default_groq_url() -> String {
    "https://api.groq.com/openai".into()
}
fn default_groq_model() -> String {
    "openai/gpt-oss-120b".into()
}
fn default_groq_models() -> Vec<String> {
    vec![
        "openai/gpt-oss-120b".into(),
        "openai/gpt-oss-20b".into(),
        "groq/compound".into(),
        "groq/compound-mini".into(),
        "llama-3.3-70b-versatile".into(),
        "whisper-large-v3-turbo".into(),
    ]
}

/// Cloudflare Workers AI — inactive lane until CLOUDFLARE_ENABLED=true and
/// CLOUDFLARE_ACCOUNT_ID + CLOUDFLARE_API_TOKEN are set.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CloudflareConfig {
    pub enabled: bool,
    pub api_key: Option<String>,
    pub account_id: Option<String>,
    #[serde(default = "default_cloudflare_model")]
    pub default_model: String,
    #[serde(default = "default_cloudflare_models")]
    pub available_models: Vec<String>,
}

fn default_cloudflare_model() -> String {
    "@cf/meta/llama-3.3-70b-instruct-fp8-fast".into()
}
fn default_cloudflare_models() -> Vec<String> {
    vec![
        "@cf/meta/llama-3.3-70b-instruct-fp8-fast".into(),
        "@cf/meta/llama-3.1-8b-instruct-fast".into(),
    ]
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ByoKeyConfig {
    pub enabled: bool,
    // BYOKEY: caller supplies their own provider + api key per request
}

impl GatewayConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        // Load .env if present
        let _ = dotenvy::dotenv();

        let gateway_key = std::env::var("GATEWAY_API_KEY")
            .unwrap_or_else(|_| "change-me-in-production".into());

        let allowed_keys: Vec<String> = std::env::var("ALLOWED_KEYS")
            .unwrap_or_default()
            .split(',')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();

        let daily_budget: f64 = std::env::var("DAILY_BUDGET_USD")
            .unwrap_or_else(|_| "10.0".into())
            .parse()
            .unwrap_or(10.0);

        let default_provider = std::env::var("DEFAULT_PROVIDER")
            .unwrap_or_else(|_| "anthropic".into());

        let fallback_chain: Vec<String> = std::env::var("FALLBACK_CHAIN")
            .unwrap_or_else(|_| "anthropic,openai,gemini".into())
            .split(',')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();

        // Build aliases map
        let mut aliases = HashMap::new();
        aliases.insert("fast".into(), "anthropic/claude-haiku-4-5-20251001".into());
        aliases.insert("smart".into(), "anthropic/claude-sonnet-4-20250514".into());
        aliases.insert("research".into(), "anthropic/claude-opus-4-20250514".into());
        aliases.insert("mercury".into(), "inception/mercury-2".into());
        aliases.insert("local".into(), "ollama/llama3.2".into());
        aliases.insert("cheap".into(), "openai/gpt-4o-mini".into());
        // Free-tier lanes (verified $0 on the providers' free plans)
        aliases.insert("free".into(), "groq/openai/gpt-oss-120b".into());
        aliases.insert("free-compound".into(), "groq/groq/compound".into());

        // Providers — only configure if keys are present
        let anthropic = {
            let key = std::env::var("ANTHROPIC_API_KEY").ok();
            let enabled = std::env::var("ANTHROPIC_ENABLED")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(key.is_some());
            if enabled || key.is_some() {
                Some(AnthropicConfig {
                    enabled,
                    api_key: key,
                    base_url: std::env::var("ANTHROPIC_BASE_URL")
                        .unwrap_or_else(|_| default_anthropic_url()),
                    default_model: std::env::var("ANTHROPIC_DEFAULT_MODEL")
                        .unwrap_or_else(|_| default_anthropic_model()),
                    available_models: default_anthropic_models(),
                })
            } else {
                None
            }
        };

        let openai = {
            let key = std::env::var("OPENAI_API_KEY").ok();
            let enabled = std::env::var("OPENAI_ENABLED")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(key.is_some());
            if enabled || key.is_some() {
                Some(OpenAIConfig {
                    enabled,
                    api_key: key,
                    base_url: std::env::var("OPENAI_BASE_URL")
                        .unwrap_or_else(|_| default_openai_url()),
                    default_model: std::env::var("OPENAI_DEFAULT_MODEL")
                        .unwrap_or_else(|_| default_openai_model()),
                    available_models: default_openai_models(),
                })
            } else {
                None
            }
        };

        let gemini = {
            let key = std::env::var("GEMINI_API_KEY").ok();
            let enabled = std::env::var("GEMINI_ENABLED")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(key.is_some());
            if enabled || key.is_some() {
                Some(GeminiConfig {
                    enabled,
                    api_key: key,
                    base_url: std::env::var("GEMINI_BASE_URL")
                        .unwrap_or_else(|_| default_gemini_url()),
                    default_model: std::env::var("GEMINI_DEFAULT_MODEL")
                        .unwrap_or_else(|_| default_gemini_model()),
                    available_models: default_gemini_models(),
                })
            } else {
                None
            }
        };

        let nvidia = {
            let key = std::env::var("NVIDIA_API_KEY").ok();
            let enabled = std::env::var("NVIDIA_ENABLED")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(key.is_some());
            if enabled || key.is_some() {
                Some(NvidiaConfig {
                    enabled,
                    api_key: key,
                    base_url: std::env::var("NVIDIA_BASE_URL")
                        .unwrap_or_else(|_| default_nvidia_url()),
                    default_model: std::env::var("NVIDIA_DEFAULT_MODEL")
                        .unwrap_or_else(|_| default_nvidia_model()),
                    available_models: default_nvidia_models(),
                })
            } else {
                None
            }
        };

        let inception = {
            let key = std::env::var("INCEPTION_API_KEY").ok();
            let enabled = std::env::var("INCEPTION_ENABLED")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false);
            Some(InceptionConfig {
                enabled,
                api_key: key,
                base_url: std::env::var("INCEPTION_BASE_URL")
                    .unwrap_or_else(|_| default_inception_url()),
                default_model: default_inception_model(),
                available_models: default_inception_models(),
            })
        };

        let zai = {
            let key = std::env::var("ZAI_API_KEY").ok();
            let enabled = std::env::var("ZAI_ENABLED")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(key.is_some());
            if enabled || key.is_some() {
                Some(ZaiConfig {
                    enabled,
                    api_key: key,
                    base_url: std::env::var("ZAI_BASE_URL")
                        .unwrap_or_else(|_| default_zai_url()),
                    default_model: default_zai_model(),
                    available_models: default_zai_models(),
                })
            } else {
                None
            }
        };

        let ollama = {
            let enabled = std::env::var("OLLAMA_ENABLED")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false);
            if enabled {
                Some(OllamaConfig {
                    enabled,
                    base_url: std::env::var("OLLAMA_BASE_URL")
                        .unwrap_or_else(|_| default_ollama_url()),
                    default_model: std::env::var("OLLAMA_DEFAULT_MODEL")
                        .unwrap_or_else(|_| default_ollama_model()),
                    available_models: default_ollama_models(),
                })
            } else {
                None
            }
        };

        let openrouter = {
            let key = std::env::var("OPENROUTER_API_KEY").ok();
            let enabled = std::env::var("OPENROUTER_ENABLED")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(key.is_some());
            if enabled || key.is_some() {
                Some(OpenRouterConfig {
                    enabled,
                    api_key: key,
                    base_url: std::env::var("OPENROUTER_BASE_URL")
                        .unwrap_or_else(|_| default_openrouter_url()),
                    default_model: default_openrouter_model(),
                    available_models: vec![],
                })
            } else {
                None
            }
        };

        let groq = {
            // StarNet stores the valid key as GROQ_API_TOKEN; accept both.
            let key = std::env::var("GROQ_API_KEY")
                .ok()
                .or_else(|| std::env::var("GROQ_API_TOKEN").ok());
            let enabled = std::env::var("GROQ_ENABLED")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(key.is_some());
            if enabled || key.is_some() {
                Some(GroqConfig {
                    enabled,
                    api_key: key,
                    base_url: std::env::var("GROQ_BASE_URL")
                        .unwrap_or_else(|_| default_groq_url()),
                    default_model: std::env::var("GROQ_DEFAULT_MODEL")
                        .unwrap_or_else(|_| default_groq_model()),
                    available_models: default_groq_models(),
                })
            } else {
                None
            }
        };

        let cloudflare = {
            // Deliberately opt-in only: a key/account without CLOUDFLARE_ENABLED
            // stays dark until the lane is deliberately turned on.
            let enabled = std::env::var("CLOUDFLARE_ENABLED")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false);
            if enabled {
                Some(CloudflareConfig {
                    enabled,
                    api_key: std::env::var("CLOUDFLARE_API_TOKEN").ok(),
                    account_id: std::env::var("CLOUDFLARE_ACCOUNT_ID").ok(),
                    default_model: std::env::var("CLOUDFLARE_DEFAULT_MODEL")
                        .unwrap_or_else(|_| default_cloudflare_model()),
                    available_models: default_cloudflare_models(),
                })
            } else {
                None
            }
        };

        let byokey = {
            let enabled = std::env::var("BYOKEY_ENABLED")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false);
            if enabled {
                Some(ByoKeyConfig { enabled })
            } else {
                None
            }
        };

        let trains_on_inputs_providers: Vec<String> = std::env::var("TRAINS_ON_INPUT_PROVIDERS")
            .map(|v| {
                v.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_else(|_| SafetyConfig::default().trains_on_inputs_providers);

        Ok(GatewayConfig {
            auth: AuthConfig {
                gateway_key,
                allowed_keys,
            },
            routing: RoutingConfig {
                default_provider,
                fallback_chain,
                aliases,
            },
            circuit_breaker: CircuitBreakerConfig {
                daily_budget_usd: daily_budget,
                failure_threshold: std::env::var("FAILURE_THRESHOLD")
                    .unwrap_or_else(|_| "5".into())
                    .parse()
                    .unwrap_or(5),
                reset_seconds: std::env::var("CIRCUIT_RESET_SECONDS")
                    .unwrap_or_else(|_| "60".into())
                    .parse()
                    .unwrap_or(60),
            },
            safety: SafetyConfig {
                trains_on_inputs_providers,
            },
            providers: ProvidersConfig {
                anthropic,
                openai,
                gemini,
                nvidia,
                inception,
                zai,
                ollama,
                openrouter,
                groq,
                cloudflare,
                byokey,
            },
        })
    }

    pub fn active_provider_count(&self) -> usize {
        [
            self.providers.anthropic.as_ref().map(|p| p.enabled).unwrap_or(false),
            self.providers.openai.as_ref().map(|p| p.enabled).unwrap_or(false),
            self.providers.gemini.as_ref().map(|p| p.enabled).unwrap_or(false),
            self.providers.nvidia.as_ref().map(|p| p.enabled).unwrap_or(false),
            self.providers.inception.as_ref().map(|p| p.enabled).unwrap_or(false),
            self.providers.zai.as_ref().map(|p| p.enabled).unwrap_or(false),
            self.providers.ollama.as_ref().map(|p| p.enabled).unwrap_or(false),
            self.providers.openrouter.as_ref().map(|p| p.enabled).unwrap_or(false),
            self.providers.groq.as_ref().map(|p| p.enabled).unwrap_or(false),
            self.providers.cloudflare.as_ref().map(|p| p.enabled).unwrap_or(false),
            self.providers.byokey.as_ref().map(|p| p.enabled).unwrap_or(false),
        ]
        .iter()
        .filter(|&&enabled| enabled)
        .count()
    }
    /// Is this provider configured *and* enabled?
    pub fn provider_enabled(&self, provider: &str) -> bool {
        match provider {
            "anthropic" => self.providers.anthropic.as_ref().map(|p| p.enabled),
            "openai" => self.providers.openai.as_ref().map(|p| p.enabled),
            "gemini" => self.providers.gemini.as_ref().map(|p| p.enabled),
            "nvidia" => self.providers.nvidia.as_ref().map(|p| p.enabled),
            "inception" => self.providers.inception.as_ref().map(|p| p.enabled),
            "zai" => self.providers.zai.as_ref().map(|p| p.enabled),
            "ollama" => self.providers.ollama.as_ref().map(|p| p.enabled),
            "openrouter" => self.providers.openrouter.as_ref().map(|p| p.enabled),
            "groq" => self.providers.groq.as_ref().map(|p| p.enabled),
            "cloudflare" => self.providers.cloudflare.as_ref().map(|p| p.enabled),
            "byokey" => self.providers.byokey.as_ref().map(|p| p.enabled),
            _ => None,
        }
        .unwrap_or(false)
    }

    /// Does this provider have the credentials its adapter needs?
    /// (Ollama needs no key; Cloudflare needs account id + token.)
    pub fn provider_has_credentials(&self, provider: &str) -> bool {
        match provider {
            "ollama" => self.providers.ollama.is_some(),
            "cloudflare" => self
                .providers
                .cloudflare
                .as_ref()
                .map(|p| p.api_key.is_some() && p.account_id.is_some())
                .unwrap_or(false),
            "byokey" => true,
            "anthropic" => self.providers.anthropic.as_ref().map(|p| p.api_key.is_some()).unwrap_or(false),
            "openai" => self.providers.openai.as_ref().map(|p| p.api_key.is_some()).unwrap_or(false),
            "gemini" => self.providers.gemini.as_ref().map(|p| p.api_key.is_some()).unwrap_or(false),
            "nvidia" => self.providers.nvidia.as_ref().map(|p| p.api_key.is_some()).unwrap_or(false),
            "inception" => self.providers.inception.as_ref().map(|p| p.api_key.is_some()).unwrap_or(false),
            "zai" => self.providers.zai.as_ref().map(|p| p.api_key.is_some()).unwrap_or(false),
            "openrouter" => self.providers.openrouter.as_ref().map(|p| p.api_key.is_some()).unwrap_or(false),
            "groq" => self.providers.groq.as_ref().map(|p| p.api_key.is_some()).unwrap_or(false),
            _ => false,
        }
    }

    /// The configured default model for a provider, when configured.
    pub fn default_model_for(&self, provider: &str) -> Option<String> {
        match provider {
            "anthropic" => self.providers.anthropic.as_ref().map(|p| p.default_model.clone()),
            "openai" => self.providers.openai.as_ref().map(|p| p.default_model.clone()),
            "gemini" => self.providers.gemini.as_ref().map(|p| p.default_model.clone()),
            "nvidia" => self.providers.nvidia.as_ref().map(|p| p.default_model.clone()),
            "inception" => self.providers.inception.as_ref().map(|p| p.default_model.clone()),
            "zai" => self.providers.zai.as_ref().map(|p| p.default_model.clone()),
            "ollama" => self.providers.ollama.as_ref().map(|p| p.default_model.clone()),
            "openrouter" => self.providers.openrouter.as_ref().map(|p| p.default_model.clone()),
            "groq" => self.providers.groq.as_ref().map(|p| p.default_model.clone()),
            "cloudflare" => self.providers.cloudflare.as_ref().map(|p| p.default_model.clone()),
            _ => None,
        }
    }

    /// Providers that may train on inputs are restricted to explicitly
    /// non-confidential traffic.
    pub fn is_restricted_provider(&self, provider: &str) -> bool {
        self.safety
            .trains_on_inputs_providers
            .iter()
            .any(|p| p == provider)
    }
}
