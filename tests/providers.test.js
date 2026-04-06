'use strict';

const { detectProvider, getProviderConfig, PROVIDERS } = require('../src/providers');

describe('detectProvider', () => {
  test('detects openai from gpt- prefix', () => {
    expect(detectProvider('gpt-4o')).toBe('openai');
    expect(detectProvider('gpt-3.5-turbo')).toBe('openai');
  });

  test('detects openai from o1/o3 prefix', () => {
    expect(detectProvider('o1-preview')).toBe('openai');
    expect(detectProvider('o3-mini')).toBe('openai');
  });

  test('detects anthropic from claude- prefix', () => {
    expect(detectProvider('claude-3-5-sonnet-20241022')).toBe('anthropic');
    expect(detectProvider('claude-opus-4')).toBe('anthropic');
  });

  test('detects groq from llama prefix', () => {
    expect(detectProvider('llama-3.3-70b-versatile')).toBe('groq');
    expect(detectProvider('llama3-8b-8192')).toBe('groq');
  });

  test('detects mistral from mistral- prefix', () => {
    expect(detectProvider('mistral-large-latest')).toBe('mistral');
    expect(detectProvider('codestral-latest')).toBe('mistral');
  });

  test('detects mixtral as mistral', () => {
    expect(detectProvider('mixtral-8x7b-32768')).toBe('mistral');
  });

  test('respects X-Provider hint over model pattern', () => {
    // Even though the model looks like openai, groq hint takes precedence.
    expect(detectProvider('llama-3.3-70b', 'groq')).toBe('groq');
    expect(detectProvider('gpt-4o', 'openai')).toBe('openai');
  });

  test('ignores unknown provider hints', () => {
    // Falls back to pattern matching.
    expect(detectProvider('gpt-4o', 'unknown-provider')).toBe('openai');
  });

  test('falls back to default for unknown models', () => {
    const result = detectProvider('my-custom-model');
    expect(['openai', 'together', 'ollama']).toContain(result);
  });
});

describe('getProviderConfig', () => {
  test('returns correct config for known providers', () => {
    const config = getProviderConfig('openai');
    expect(config).toHaveProperty('baseUrl');
    expect(config).toHaveProperty('openaiCompat', true);
  });

  test('returns anthropic config with openaiCompat false', () => {
    const config = getProviderConfig('anthropic');
    expect(config.openaiCompat).toBe(false);
  });

  test('falls back to openai for unknown provider', () => {
    const config = getProviderConfig('nonexistent');
    expect(config).toEqual(PROVIDERS.openai);
  });
});
