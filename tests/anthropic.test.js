'use strict';

const {
  toAnthropicRequest,
  fromAnthropicResponse,
  transformAnthropicStreamEvent,
} = require('../src/adapters/anthropic');

describe('toAnthropicRequest', () => {
  const apiKey = 'sk-ant-test';

  test('converts a basic user message', () => {
    const { body, headers } = toAnthropicRequest(
      {
        model: 'claude-3-5-sonnet-20241022',
        messages: [{ role: 'user', content: 'Hello' }],
      },
      apiKey,
    );

    expect(body.model).toBe('claude-3-5-sonnet-20241022');
    expect(body.messages).toEqual([{ role: 'user', content: 'Hello' }]);
    expect(body.max_tokens).toBe(4096);
    expect(headers['x-api-key']).toBe(apiKey);
    expect(headers['anthropic-version']).toBeDefined();
  });

  test('extracts system message into top-level system field', () => {
    const { body } = toAnthropicRequest(
      {
        model: 'claude-opus-4',
        messages: [
          { role: 'system', content: 'You are a helpful assistant.' },
          { role: 'user', content: 'Hi' },
        ],
      },
      apiKey,
    );

    expect(body.system).toBe('You are a helpful assistant.');
    expect(body.messages).toHaveLength(1);
    expect(body.messages[0].role).toBe('user');
  });

  test('respects caller-supplied max_tokens', () => {
    const { body } = toAnthropicRequest(
      { model: 'claude-3-5-sonnet-20241022', messages: [], max_tokens: 100 },
      apiKey,
    );
    expect(body.max_tokens).toBe(100);
  });

  test('converts stop string to stop_sequences array', () => {
    const { body } = toAnthropicRequest(
      { model: 'claude-3-5-sonnet-20241022', messages: [], stop: 'STOP' },
      apiKey,
    );
    expect(body.stop_sequences).toEqual(['STOP']);
  });

  test('keeps stop_sequences array as-is', () => {
    const { body } = toAnthropicRequest(
      {
        model: 'claude-3-5-sonnet-20241022',
        messages: [],
        stop: ['END', 'STOP'],
      },
      apiKey,
    );
    expect(body.stop_sequences).toEqual(['END', 'STOP']);
  });

  test('enables streaming when stream=true', () => {
    const { body } = toAnthropicRequest(
      { model: 'claude-3-5-sonnet-20241022', messages: [], stream: true },
      apiKey,
    );
    expect(body.stream).toBe(true);
  });

  test('passes through temperature and top_p', () => {
    const { body } = toAnthropicRequest(
      {
        model: 'claude-3-5-sonnet-20241022',
        messages: [],
        temperature: 0.7,
        top_p: 0.9,
      },
      apiKey,
    );
    expect(body.temperature).toBe(0.7);
    expect(body.top_p).toBe(0.9);
  });
});

describe('fromAnthropicResponse', () => {
  const anthropicResponse = {
    id: 'msg_01',
    model: 'claude-3-5-sonnet-20241022',
    stop_reason: 'end_turn',
    content: [{ type: 'text', text: 'Hello there!' }],
    usage: { input_tokens: 10, output_tokens: 5 },
  };

  test('converts to OpenAI format', () => {
    const result = fromAnthropicResponse(anthropicResponse);

    expect(result.id).toBe('msg_01');
    expect(result.object).toBe('chat.completion');
    expect(result.model).toBe('claude-3-5-sonnet-20241022');
    expect(result.choices).toHaveLength(1);
    expect(result.choices[0].message.role).toBe('assistant');
    expect(result.choices[0].message.content).toBe('Hello there!');
    expect(result.choices[0].finish_reason).toBe('stop');
  });

  test('maps usage correctly', () => {
    const result = fromAnthropicResponse(anthropicResponse);
    expect(result.usage.prompt_tokens).toBe(10);
    expect(result.usage.completion_tokens).toBe(5);
    expect(result.usage.total_tokens).toBe(15);
  });

  test('maps max_tokens stop_reason to "length"', () => {
    const result = fromAnthropicResponse({
      ...anthropicResponse,
      stop_reason: 'max_tokens',
    });
    expect(result.choices[0].finish_reason).toBe('length');
  });
});

describe('transformAnthropicStreamEvent', () => {
  test('message_start emits initial chunk with role', () => {
    const state = {};
    const out = transformAnthropicStreamEvent(
      'message_start',
      { message: { id: 'msg_01', model: 'claude-3-5-sonnet-20241022' } },
      state,
    );

    expect(state.id).toBe('msg_01');
    expect(state.model).toBe('claude-3-5-sonnet-20241022');

    const parsed = JSON.parse(out.replace('data: ', '').trim());
    expect(parsed.choices[0].delta.role).toBe('assistant');
    expect(parsed.choices[0].delta.content).toBeUndefined();
  });

  test('content_block_delta emits text chunk', () => {
    const state = { id: 'msg_01', model: 'claude-3-5-sonnet-20241022' };
    const out = transformAnthropicStreamEvent(
      'content_block_delta',
      { delta: { type: 'text_delta', text: 'Hello' } },
      state,
    );

    const parsed = JSON.parse(out.replace('data: ', '').trim());
    expect(parsed.choices[0].delta.content).toBe('Hello');
  });

  test('content_block_delta returns null for empty text', () => {
    const state = {};
    const out = transformAnthropicStreamEvent(
      'content_block_delta',
      { delta: { text: '' } },
      state,
    );
    expect(out).toBeNull();
  });

  test('message_delta emits finish_reason chunk', () => {
    const state = { id: 'msg_01', model: 'claude-3-5-sonnet-20241022' };
    const out = transformAnthropicStreamEvent(
      'message_delta',
      { delta: { stop_reason: 'end_turn' } },
      state,
    );

    const parsed = JSON.parse(out.replace('data: ', '').trim());
    expect(parsed.choices[0].finish_reason).toBe('stop');
  });

  test('message_stop emits [DONE]', () => {
    const out = transformAnthropicStreamEvent('message_stop', {}, {});
    expect(out).toBe('data: [DONE]\n\n');
  });

  test('unknown events return null', () => {
    const out = transformAnthropicStreamEvent('ping', {}, {});
    expect(out).toBeNull();
  });
});
