'use strict';

/**
 * Anthropic ↔ OpenAI format adapter.
 *
 * Translates OpenAI-style chat-completion requests to the Anthropic Messages
 * API and translates responses (both streaming and non-streaming) back to the
 * OpenAI format so that any OpenAI-compatible AI agent can use Anthropic
 * models transparently.
 */

const ANTHROPIC_VERSION = '2023-06-01';
const DEFAULT_MAX_TOKENS = 4096;

/**
 * Transform an OpenAI chat-completion request body into an Anthropic
 * Messages API request body, and return the headers needed for the call.
 *
 * @param {object} openaiBody - Parsed OpenAI request body.
 * @param {string} apiKey     - Anthropic API key.
 * @returns {{ body: object, headers: object }}
 */
function toAnthropicRequest(openaiBody, apiKey) {
  const {
    model,
    messages = [],
    max_tokens,
    temperature,
    top_p,
    top_k,
    stop,
    stream,
  } = openaiBody;

  // Separate the optional system message from the conversation.
  let systemPrompt;
  const conversationMessages = [];

  for (const msg of messages) {
    if (msg.role === 'system') {
      systemPrompt = Array.isArray(msg.content)
        ? msg.content.map((c) => (typeof c === 'string' ? c : c.text)).join('\n')
        : msg.content;
    } else {
      // Convert content to the format Anthropic expects.
      const content =
        typeof msg.content === 'string'
          ? msg.content
          : Array.isArray(msg.content)
          ? msg.content.map((c) => {
              if (typeof c === 'string') return { type: 'text', text: c };
              if (c.type === 'text') return { type: 'text', text: c.text };
              if (c.type === 'image_url') {
                // Convert OpenAI image_url to Anthropic image source.
                const url = c.image_url?.url || '';
                if (url.startsWith('data:')) {
                  const [meta, data] = url.split(',');
                  const mediaType = meta.replace('data:', '').replace(';base64', '');
                  return {
                    type: 'image',
                    source: { type: 'base64', media_type: mediaType, data },
                  };
                }
                return { type: 'image', source: { type: 'url', url } };
              }
              return { type: 'text', text: JSON.stringify(c) };
            })
          : msg.content;

      conversationMessages.push({ role: msg.role, content });
    }
  }

  const body = {
    model,
    max_tokens: max_tokens || DEFAULT_MAX_TOKENS,
    messages: conversationMessages,
    stream: stream || false,
  };

  if (systemPrompt !== undefined) body.system = systemPrompt;
  if (temperature !== undefined) body.temperature = temperature;
  if (top_p !== undefined) body.top_p = top_p;
  if (top_k !== undefined) body.top_k = top_k;
  if (stop !== undefined) body.stop_sequences = Array.isArray(stop) ? stop : [stop];

  const headers = {
    'Content-Type': 'application/json',
    'x-api-key': apiKey,
    'anthropic-version': ANTHROPIC_VERSION,
  };

  return { body, headers };
}

/**
 * Transform a non-streaming Anthropic Messages response into an OpenAI
 * chat-completion response.
 *
 * @param {object} anthropicResponse
 * @returns {object} OpenAI-shaped response.
 */
function fromAnthropicResponse(anthropicResponse) {
  const content =
    anthropicResponse.content
      ?.filter((c) => c.type === 'text')
      .map((c) => c.text)
      .join('') || '';

  const finishReason =
    anthropicResponse.stop_reason === 'end_turn'
      ? 'stop'
      : anthropicResponse.stop_reason === 'max_tokens'
      ? 'length'
      : anthropicResponse.stop_reason || 'stop';

  return {
    id: anthropicResponse.id,
    object: 'chat.completion',
    created: Math.floor(Date.now() / 1000),
    model: anthropicResponse.model,
    choices: [
      {
        index: 0,
        message: { role: 'assistant', content },
        finish_reason: finishReason,
      },
    ],
    usage: {
      prompt_tokens: anthropicResponse.usage?.input_tokens || 0,
      completion_tokens: anthropicResponse.usage?.output_tokens || 0,
      total_tokens:
        (anthropicResponse.usage?.input_tokens || 0) +
        (anthropicResponse.usage?.output_tokens || 0),
    },
  };
}

/**
 * Parse a raw Anthropic SSE line buffer and emit the equivalent OpenAI-format
 * SSE chunks as a string.
 *
 * This is a streaming transformer: it receives a single `data:` payload
 * from Anthropic and returns the corresponding OpenAI SSE string(s), or
 * null if the event should be skipped.
 *
 * @param {string} eventType   - Anthropic event type (e.g. "content_block_delta").
 * @param {object} eventData   - Parsed JSON data payload.
 * @param {object} state       - Mutable state object shared across calls:
 *                               { id, model, role, finishReason }
 * @returns {string|null} SSE chunk(s) to forward, or null to skip.
 */
function transformAnthropicStreamEvent(eventType, eventData, state) {
  const chunk = (delta, finishReason = null) => {
    const payload = {
      id: state.id || `chatcmpl-${Date.now()}`,
      object: 'chat.completion.chunk',
      created: Math.floor(Date.now() / 1000),
      model: state.model || '',
      choices: [
        {
          index: 0,
          delta,
          finish_reason: finishReason,
        },
      ],
    };
    return `data: ${JSON.stringify(payload)}\n\n`;
  };

  switch (eventType) {
    case 'message_start': {
      state.id = eventData.message?.id;
      state.model = eventData.message?.model;
      // Emit an initial chunk with the role only (no content field).
      return chunk({ role: 'assistant' });
    }

    case 'content_block_delta': {
      const text = eventData.delta?.text || '';
      if (!text) return null;
      return chunk({ content: text });
    }

    case 'message_delta': {
      const stopReason = eventData.delta?.stop_reason;
      const finishReason =
        stopReason === 'end_turn'
          ? 'stop'
          : stopReason === 'max_tokens'
          ? 'length'
          : stopReason || 'stop';
      state.finishReason = finishReason;
      return chunk({ content: '' }, finishReason);
    }

    case 'message_stop':
      return 'data: [DONE]\n\n';

    default:
      return null;
  }
}

module.exports = {
  toAnthropicRequest,
  fromAnthropicResponse,
  transformAnthropicStreamEvent,
};
