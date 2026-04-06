'use strict';

const request = require('supertest');
const nock = require('nock');
const createApp = require('../src/app');

let app;

beforeAll(() => {
  // Ensure pass-through mode (no gateway key).
  delete process.env.GATEWAY_API_KEY;
  app = createApp();
});

afterEach(() => {
  nock.cleanAll();
});

// ─── Health check ─────────────────────────────────────────────────────────────

describe('GET /health', () => {
  test('returns 200 with status ok', async () => {
    const res = await request(app).get('/health');
    expect(res.status).toBe(200);
    expect(res.body.status).toBe('ok');
    expect(res.body.service).toBe('synthia-gateway');
  });
});

// ─── 404 fallback ─────────────────────────────────────────────────────────────

describe('Unknown routes', () => {
  test('returns 404 for unknown path', async () => {
    const res = await request(app).get('/unknown-path');
    expect(res.status).toBe(404);
    expect(res.body.error).toBeDefined();
  });
});

// ─── Auth middleware ───────────────────────────────────────────────────────────

describe('Gateway authentication', () => {
  let appWithKey;

  beforeAll(() => {
    process.env.GATEWAY_API_KEY = 'test-gateway-secret';
    appWithKey = createApp();
  });

  afterAll(() => {
    delete process.env.GATEWAY_API_KEY;
  });

  test('rejects requests without a valid gateway key', async () => {
    const res = await request(appWithKey)
      .post('/v1/chat/completions')
      .set('Authorization', 'Bearer wrong-key')
      .send({ model: 'gpt-4o', messages: [] });

    expect(res.status).toBe(401);
    expect(res.body.error.code).toBe('invalid_api_key');
  });

  test('allows requests with the correct gateway key', async () => {
    // Mock the upstream OpenAI call so the request doesn't fail.
    nock('https://api.openai.com')
      .post('/v1/chat/completions')
      .reply(200, {
        id: 'chatcmpl-test',
        object: 'chat.completion',
        created: 1234567890,
        model: 'gpt-4o',
        choices: [
          {
            index: 0,
            message: { role: 'assistant', content: 'Hello!' },
            finish_reason: 'stop',
          },
        ],
        usage: { prompt_tokens: 5, completion_tokens: 2, total_tokens: 7 },
      });

    process.env.OPENAI_API_KEY = 'sk-fake-key';

    const res = await request(appWithKey)
      .post('/v1/chat/completions')
      .set('Authorization', 'Bearer test-gateway-secret')
      .send({
        model: 'gpt-4o',
        messages: [{ role: 'user', content: 'Hi' }],
      });

    expect(res.status).toBe(200);
    delete process.env.OPENAI_API_KEY;
  });

  test('/health is accessible without gateway key', async () => {
    const res = await request(appWithKey).get('/health');
    expect(res.status).toBe(200);
  });
});

// ─── Proxy: OpenAI ────────────────────────────────────────────────────────────

describe('POST /v1/chat/completions → OpenAI', () => {
  test('returns 401 when no API key is provided', async () => {
    const res = await request(app)
      .post('/v1/chat/completions')
      .send({ model: 'gpt-4o', messages: [{ role: 'user', content: 'Hi' }] });

    expect(res.status).toBe(401);
    expect(res.body.error.code).toBe('missing_api_key');
  });

  test('forwards request to OpenAI and returns response', async () => {
    nock('https://api.openai.com')
      .post('/v1/chat/completions')
      .reply(200, {
        id: 'chatcmpl-abc',
        object: 'chat.completion',
        choices: [
          {
            index: 0,
            message: { role: 'assistant', content: 'Hello!' },
            finish_reason: 'stop',
          },
        ],
        usage: { prompt_tokens: 5, completion_tokens: 3, total_tokens: 8 },
      });

    const res = await request(app)
      .post('/v1/chat/completions')
      .set('Authorization', 'Bearer sk-fake-openai-key')
      .send({
        model: 'gpt-4o',
        messages: [{ role: 'user', content: 'Hello' }],
      });

    expect(res.status).toBe(200);
    expect(res.body.id).toBe('chatcmpl-abc');
    expect(res.body.choices[0].message.content).toBe('Hello!');
  });

  test('forwards upstream error status to caller', async () => {
    nock('https://api.openai.com')
      .post('/v1/chat/completions')
      .reply(429, {
        error: { message: 'Rate limit exceeded', type: 'requests', code: 'rate_limit_exceeded' },
      });

    const res = await request(app)
      .post('/v1/chat/completions')
      .set('Authorization', 'Bearer sk-fake-key')
      .send({ model: 'gpt-4o', messages: [] });

    expect(res.status).toBe(429);
  });
});

// ─── Proxy: Anthropic ─────────────────────────────────────────────────────────

describe('POST /v1/chat/completions → Anthropic', () => {
  test('transforms request and response formats', async () => {
    nock('https://api.anthropic.com')
      .post('/v1/messages')
      .reply(200, {
        id: 'msg_01',
        model: 'claude-3-5-sonnet-20241022',
        stop_reason: 'end_turn',
        content: [{ type: 'text', text: 'Hi there!' }],
        usage: { input_tokens: 8, output_tokens: 4 },
      });

    const res = await request(app)
      .post('/v1/chat/completions')
      .set('Authorization', 'Bearer sk-ant-fake-key')
      .send({
        model: 'claude-3-5-sonnet-20241022',
        messages: [{ role: 'user', content: 'Hello' }],
      });

    expect(res.status).toBe(200);
    expect(res.body.object).toBe('chat.completion');
    expect(res.body.choices[0].message.content).toBe('Hi there!');
    expect(res.body.usage.total_tokens).toBe(12);
  });
});

// ─── Proxy: embeddings ────────────────────────────────────────────────────────

describe('POST /v1/embeddings', () => {
  test('proxies embedding requests to OpenAI', async () => {
    nock('https://api.openai.com')
      .post('/v1/embeddings')
      .reply(200, {
        object: 'list',
        data: [{ object: 'embedding', embedding: [0.1, 0.2], index: 0 }],
        model: 'text-embedding-3-small',
        usage: { prompt_tokens: 5, total_tokens: 5 },
      });

    const res = await request(app)
      .post('/v1/embeddings')
      .set('Authorization', 'Bearer sk-fake-key')
      .send({ model: 'text-embedding-3-small', input: 'Hello world' });

    expect(res.status).toBe(200);
    expect(res.body.data).toHaveLength(1);
  });
});

// ─── Proxy: models list ───────────────────────────────────────────────────────

describe('GET /v1/models', () => {
  test('proxies model list request', async () => {
    nock('https://api.openai.com')
      .get('/v1/models')
      .reply(200, {
        object: 'list',
        data: [{ id: 'gpt-4o', object: 'model' }],
      });

    const res = await request(app)
      .get('/v1/models')
      .set('Authorization', 'Bearer sk-fake-key');

    expect(res.status).toBe(200);
    expect(res.body.data).toBeDefined();
  });
});

// ─── X-Provider header ────────────────────────────────────────────────────────

describe('X-Provider header routing', () => {
  test('routes to groq when X-Provider: groq is set', async () => {
    nock('https://api.groq.com')
      .post('/openai/v1/chat/completions')
      .reply(200, {
        id: 'chatcmpl-groq',
        object: 'chat.completion',
        choices: [
          {
            index: 0,
            message: { role: 'assistant', content: 'Groq response' },
            finish_reason: 'stop',
          },
        ],
        usage: { prompt_tokens: 4, completion_tokens: 2, total_tokens: 6 },
      });

    const res = await request(app)
      .post('/v1/chat/completions')
      .set('Authorization', 'Bearer gsk-fake-groq-key')
      .set('X-Provider', 'groq')
      .send({
        model: 'llama-3.3-70b-versatile',
        messages: [{ role: 'user', content: 'Hello' }],
      });

    expect(res.status).toBe(200);
    expect(res.body.choices[0].message.content).toBe('Groq response');
  });
});
