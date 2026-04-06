'use strict';

const router = require('express').Router();
const fetch = require('node-fetch');
const { detectProvider, getProviderConfig } = require('../providers');
const {
  toAnthropicRequest,
  fromAnthropicResponse,
  transformAnthropicStreamEvent,
} = require('../adapters/anthropic');

// ─── Helpers ─────────────────────────────────────────────────────────────────

/**
 * Resolve the API key to use for the outgoing request.
 *
 * In gateway-key mode (GATEWAY_API_KEY is set), the key is read from an
 * environment variable specific to each provider.
 * In pass-through mode, the caller's Authorization bearer token is used.
 *
 * @param {import('express').Request} req
 * @param {object} providerConfig
 * @returns {string|null}
 */
function resolveApiKey(req, providerConfig) {
  if (process.env.GATEWAY_API_KEY && providerConfig.envKey) {
    return process.env[providerConfig.envKey] || null;
  }

  const authHeader = req.headers['authorization'] || '';
  return authHeader.startsWith('Bearer ')
    ? authHeader.slice('Bearer '.length).trim()
    : null;
}

/**
 * Build the upstream URL for a given provider, path, and query string.
 *
 * @param {object} providerConfig
 * @param {string} path           - e.g. "/v1/chat/completions"
 * @param {string} queryString    - e.g. "?foo=bar" (may be empty)
 * @returns {string}
 */
function buildUpstreamUrl(providerConfig, path, queryString) {
  const base = providerConfig.baseUrl.replace(/\/$/, '');
  return `${base}${path}${queryString ? `?${queryString}` : ''}`;
}

/**
 * Forward headers from the incoming request to the outgoing request,
 * filtering out hop-by-hop and host-specific headers.
 *
 * @param {import('express').Request} req
 * @returns {object}
 */
function buildPassthroughHeaders(req) {
  const skip = new Set([
    'host',
    'connection',
    'authorization',
    'content-length',
    'transfer-encoding',
    'te',
    'trailers',
    'upgrade',
    'proxy-authorization',
    'proxy-connection',
    'x-provider',
  ]);

  const headers = {};
  for (const [key, value] of Object.entries(req.headers)) {
    if (!skip.has(key.toLowerCase())) {
      headers[key] = value;
    }
  }
  return headers;
}

// ─── Streaming helpers ────────────────────────────────────────────────────────

/**
 * Pipe a streaming OpenAI-compatible response back to the client.
 * Works for any provider that uses the OpenAI SSE wire format.
 */
function pipeStream(upstreamRes, res) {
  res.setHeader('Content-Type', 'text/event-stream');
  res.setHeader('Cache-Control', 'no-cache');
  res.setHeader('Connection', 'keep-alive');
  res.setHeader('X-Accel-Buffering', 'no');
  upstreamRes.body.pipe(res);
}

/**
 * Consume a streaming Anthropic response, transform each SSE event to the
 * OpenAI SSE format, and write it to the client response.
 */
async function pipeAnthropicStream(upstreamRes, res) {
  res.setHeader('Content-Type', 'text/event-stream');
  res.setHeader('Cache-Control', 'no-cache');
  res.setHeader('Connection', 'keep-alive');
  res.setHeader('X-Accel-Buffering', 'no');

  const state = { id: null, model: null, finishReason: null };
  let buffer = '';
  let currentEventType = null;

  for await (const chunk of upstreamRes.body) {
    buffer += chunk.toString();

    // Process complete SSE lines.
    const lines = buffer.split('\n');
    buffer = lines.pop(); // keep the last incomplete line

    for (const line of lines) {
      const trimmed = line.trim();

      if (trimmed.startsWith('event:')) {
        currentEventType = trimmed.slice('event:'.length).trim();
      } else if (trimmed.startsWith('data:')) {
        const rawData = trimmed.slice('data:'.length).trim();
        if (rawData === '[DONE]') {
          res.write('data: [DONE]\n\n');
          continue;
        }
        if (!currentEventType) continue;

        let eventData;
        try {
          eventData = JSON.parse(rawData);
        } catch {
          continue;
        }

        const outChunk = transformAnthropicStreamEvent(
          currentEventType,
          eventData,
          state,
        );
        if (outChunk) res.write(outChunk);

        if (currentEventType === 'message_stop') {
          res.end();
          return;
        }
      } else if (trimmed === '') {
        currentEventType = null;
      }
    }
  }

  res.end();
}

// ─── Route handler factory ────────────────────────────────────────────────────

/**
 * Generic proxy handler for all LLM endpoints.
 *
 * @param {string} method  - HTTP method to use upstream ("POST" | "GET").
 */
function proxyHandler(method) {
  return async (req, res) => {
    const body = req.body;
    const model = body?.model || '';
    const providerHint = req.headers['x-provider'] || null;

    const providerName = detectProvider(model, providerHint);
    const providerConfig = getProviderConfig(providerName);
    const isStreaming = body?.stream === true;

    const apiKey = resolveApiKey(req, providerConfig);
    if (!apiKey && providerName !== 'ollama') {
      return res.status(401).json({
        error: {
          message:
            'No API key provided. Pass your provider key as a Bearer token ' +
            'or set the appropriate *_API_KEY environment variable.',
          type: 'authentication_error',
          code: 'missing_api_key',
        },
      });
    }

    try {
      if (providerName === 'anthropic') {
        // ── Anthropic path ────────────────────────────────────────────────
        const { body: anthropicBody, headers: anthropicHeaders } =
          toAnthropicRequest(body, apiKey);

        const url = `${providerConfig.baseUrl.replace(/\/$/, '')}/v1/messages`;

        const upstreamRes = await fetch(url, {
          method: 'POST',
          headers: anthropicHeaders,
          body: JSON.stringify(anthropicBody),
        });

        if (!upstreamRes.ok && !isStreaming) {
          const errorText = await upstreamRes.text();
          let errorJson;
          try {
            errorJson = JSON.parse(errorText);
          } catch {
            errorJson = { error: { message: errorText } };
          }
          return res.status(upstreamRes.status).json(errorJson);
        }

        if (isStreaming) {
          return pipeAnthropicStream(upstreamRes, res);
        }

        const anthropicResponse = await upstreamRes.json();
        return res.json(fromAnthropicResponse(anthropicResponse));
      }

      // ── OpenAI-compatible path ─────────────────────────────────────────
      const queryString = new URLSearchParams(req.query).toString();
      const url = buildUpstreamUrl(providerConfig, req.path, queryString);

      const passthroughHeaders = buildPassthroughHeaders(req);
      const outgoingHeaders = {
        ...passthroughHeaders,
        'Content-Type': 'application/json',
        ...(apiKey ? { Authorization: `Bearer ${apiKey}` } : {}),
      };

      const fetchOptions = {
        method,
        headers: outgoingHeaders,
      };

      if (method !== 'GET' && body) {
        fetchOptions.body = JSON.stringify(body);
      }

      const upstreamRes = await fetch(url, fetchOptions);

      if (isStreaming && upstreamRes.ok) {
        return pipeStream(upstreamRes, res);
      }

      // Non-streaming: forward status and body.
      const responseBody = await upstreamRes.text();
      res.status(upstreamRes.status);

      const contentType = upstreamRes.headers.get('content-type') || '';
      if (contentType.includes('application/json')) {
        res.setHeader('Content-Type', 'application/json');
        return res.send(responseBody);
      }

      return res.send(responseBody);
    } catch (err) {
      // Network / unexpected errors.
      const status = err.code === 'ECONNREFUSED' ? 502 : 500;
      return res.status(status).json({
        error: {
          message: err.message || 'Internal proxy error.',
          type: 'proxy_error',
          code: err.code || 'internal_error',
        },
      });
    }
  };
}

// ─── Routes ───────────────────────────────────────────────────────────────────

// Chat completions (the primary endpoint used by AI agents)
router.post('/v1/chat/completions', proxyHandler('POST'));

// Legacy completions
router.post('/v1/completions', proxyHandler('POST'));

// Embeddings
router.post('/v1/embeddings', proxyHandler('POST'));

// Models list — forwarded to the detected / default provider.
// The caller may pass X-Provider to specify which provider's model list to fetch.
router.get('/v1/models', async (req, res) => {
  const providerHint = req.headers['x-provider'] || null;
  const providerName = detectProvider('', providerHint);
  const providerConfig = getProviderConfig(providerName);
  const apiKey = resolveApiKey(req, providerConfig);

  try {
    const url = `${providerConfig.baseUrl.replace(/\/$/, '')}/v1/models`;
    const headers = { 'Content-Type': 'application/json' };
    if (apiKey) headers['Authorization'] = `Bearer ${apiKey}`;

    const upstreamRes = await fetch(url, { method: 'GET', headers });
    const body = await upstreamRes.text();
    res.status(upstreamRes.status);
    res.setHeader('Content-Type', 'application/json');
    res.send(body);
  } catch (err) {
    const status = err.code === 'ECONNREFUSED' ? 502 : 500;
    res.status(status).json({
      error: {
        message: err.message || 'Internal proxy error.',
        type: 'proxy_error',
        code: err.code || 'internal_error',
      },
    });
  }
});

module.exports = router;
