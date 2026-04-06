# synthia-gateway

A **Bring Your Own Key (BYOK)** proxy that connects AI agents to LLM
subscriptions.  
AI agents send standard OpenAI-compatible requests to the gateway; the gateway
routes them to the right LLM provider (OpenAI, Anthropic, Groq, Mistral,
Together AI, Ollama, …) using the caller-supplied API key or server-side keys
you configure once.

---

## Features

| Feature | Detail |
|---|---|
| **OpenAI-compatible API** | Drop-in replacement — point any OpenAI SDK at the gateway URL |
| **Multi-provider routing** | Auto-detects the provider from the model name; overridable with `X-Provider` header |
| **Anthropic support** | Full OpenAI ↔ Anthropic format translation, including streaming SSE |
| **Two auth modes** | Pass-through (caller sends their own key) or gateway-key (keys stored server-side) |
| **Streaming** | Server-Sent Events (SSE) supported for all providers |
| **Embeddings & models** | `/v1/embeddings` and `/v1/models` endpoints proxied |

---

## Supported providers

| Provider | Model prefix (auto-detect) | Override with `X-Provider` |
|---|---|---|
| OpenAI | `gpt-`, `o1`, `o3`, `text-embedding-`, `dall-e-`, `whisper-`, `tts-` | `openai` |
| Anthropic | `claude-` | `anthropic` |
| Groq | `llama`, `gemma` | `groq` |
| Mistral / Mixtral | `mistral-`, `mixtral-`, `codestral-` | `mistral` |
| Together AI | _(set `X-Provider: together`)_ | `together` |
| Ollama (local) | _(set `X-Provider: ollama`)_ | `ollama` |

---

## Quick start

```bash
# 1. Clone and install
git clone https://github.com/executiveusa/synthia-gateway
cd synthia-gateway
npm install

# 2. Configure
cp .env.example .env
# Edit .env — at minimum set PORT and (if using gateway-key mode) your keys.

# 3. Start
npm start
# → synthia-gateway listening on port 3000
```

### Using with the OpenAI SDK

```python
from openai import OpenAI

client = OpenAI(
    base_url="http://localhost:3000/v1",
    api_key="sk-your-openai-key",   # your real provider key (pass-through mode)
)

response = client.chat.completions.create(
    model="gpt-4o",
    messages=[{"role": "user", "content": "Hello!"}],
)
print(response.choices[0].message.content)
```

### Using Anthropic via the OpenAI SDK

```python
response = client.chat.completions.create(
    model="claude-3-5-sonnet-20241022",
    messages=[{"role": "user", "content": "Hello!"}],
    extra_headers={"Authorization": "Bearer sk-ant-your-anthropic-key"},
)
```

---

## Configuration

Copy `.env.example` to `.env` and set the relevant variables.

### Pass-through mode (default)

Leave `GATEWAY_API_KEY` unset.  Every request must carry the caller's provider
API key as an `Authorization: Bearer <key>` header.  No keys are stored on the
server.

### Gateway-key mode

Set `GATEWAY_API_KEY` to a secret shared with your AI agents.  Store provider
keys in the corresponding environment variables.  Callers authenticate with
only the gateway key.

```
GATEWAY_API_KEY=my-shared-secret

OPENAI_API_KEY=sk-...
ANTHROPIC_API_KEY=sk-ant-...
GROQ_API_KEY=gsk_...
MISTRAL_API_KEY=...
```

### Custom provider base URLs

Override any provider's base URL — useful for Azure OpenAI, LM Studio, or
other self-hosted endpoints:

```
OPENAI_BASE_URL=https://<resource>.openai.azure.com
OLLAMA_BASE_URL=http://gpu-server:11434
```

---

## API

All endpoints mirror the OpenAI API.  Set `base_url` in your client to point
at the gateway.

| Method | Path | Description |
|---|---|---|
| `GET` | `/health` | Liveness check (no auth required) |
| `POST` | `/v1/chat/completions` | Chat completions (streaming supported) |
| `POST` | `/v1/completions` | Legacy text completions |
| `POST` | `/v1/embeddings` | Embeddings |
| `GET` | `/v1/models` | List available models |

### `X-Provider` header

Force routing to a specific provider regardless of the model name:

```
X-Provider: groq
```

---

## Development

```bash
npm run dev      # start with nodemon (auto-restart)
npm test         # run test suite
npm run lint     # ESLint
```

---

## License

MIT
