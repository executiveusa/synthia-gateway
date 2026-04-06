'use strict';

/**
 * Provider registry — maps model-name patterns to provider configurations.
 *
 * Each provider entry:
 *   baseUrl     : Default upstream base URL (can be overridden via env vars).
 *   pattern     : RegExp tested against the requested model name.
 *   openaiCompat: true  → request/response are forwarded verbatim.
 *                 false → an adapter performs format translation.
 *   envKey      : Name of the env-var that holds the API key for this provider
 *                 (used only when GATEWAY_API_KEY is set).
 *   baseUrlEnv  : Name of the env-var that holds the custom base URL.
 */
const PROVIDERS = {
  openai: {
    baseUrl: process.env.OPENAI_BASE_URL || 'https://api.openai.com',
    pattern: /^(gpt-|o[0-9]|text-embedding-|dall-e-|whisper-|tts-)/,
    openaiCompat: true,
    envKey: 'OPENAI_API_KEY',
    baseUrlEnv: 'OPENAI_BASE_URL',
  },
  anthropic: {
    baseUrl: process.env.ANTHROPIC_BASE_URL || 'https://api.anthropic.com',
    pattern: /^claude-/,
    openaiCompat: false,
    envKey: 'ANTHROPIC_API_KEY',
    baseUrlEnv: 'ANTHROPIC_BASE_URL',
  },
  groq: {
    baseUrl: process.env.GROQ_BASE_URL || 'https://api.groq.com/openai',
    pattern: /^(llama[0-9-]|llama3|gemma[0-9-]|whisper-large)/i,
    openaiCompat: true,
    envKey: 'GROQ_API_KEY',
    baseUrlEnv: 'GROQ_BASE_URL',
  },
  mistral: {
    baseUrl: process.env.MISTRAL_BASE_URL || 'https://api.mistral.ai',
    pattern: /^(mistral-|mixtral-|codestral-)/i,
    openaiCompat: true,
    envKey: 'MISTRAL_API_KEY',
    baseUrlEnv: 'MISTRAL_BASE_URL',
  },
  together: {
    baseUrl: process.env.TOGETHER_BASE_URL || 'https://api.together.xyz',
    pattern: null,
    openaiCompat: true,
    envKey: 'TOGETHER_API_KEY',
    baseUrlEnv: 'TOGETHER_BASE_URL',
  },
  ollama: {
    baseUrl: process.env.OLLAMA_BASE_URL || 'http://localhost:11434',
    pattern: null,
    openaiCompat: true,
    envKey: null,
    baseUrlEnv: 'OLLAMA_BASE_URL',
  },
};

/**
 * Detect which provider should handle a request.
 *
 * @param {string}      model        - The model name from the request body.
 * @param {string|null} providerHint - Value of the X-Provider request header.
 * @returns {string} Provider name key (matches a key in PROVIDERS).
 */
function detectProvider(model, providerHint) {
  if (providerHint) {
    const hint = providerHint.toLowerCase().trim();
    if (PROVIDERS[hint]) return hint;
  }

  for (const [name, config] of Object.entries(PROVIDERS)) {
    if (config.pattern && config.pattern.test(model)) {
      return name;
    }
  }

  return (process.env.DEFAULT_PROVIDER || 'openai').toLowerCase();
}

/**
 * Return the configuration object for a provider.
 * Falls back to openai if the name is unknown.
 *
 * @param {string} providerName
 * @returns {object}
 */
function getProviderConfig(providerName) {
  return PROVIDERS[providerName] || PROVIDERS.openai;
}

module.exports = { PROVIDERS, detectProvider, getProviderConfig };
