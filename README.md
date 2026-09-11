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

---

## Rust gateway (what Docker / nixpacks actually deploys — port 8018)

The Rust binary adds spend tracking, a daily budget breaker, provider health
circuits, executable fallback chains, and data-safety routing on top of the
same OpenAI-compatible surface.

### Provider env vars

| Variable | Notes |
|---|---|
| `GROQ_API_KEY` / `GROQ_API_TOKEN` | Either name works. Enables the Groq lane (default model `openai/gpt-oss-120b`). |
| `GROQ_BASE_URL` | Defaults to `https://api.groq.com/openai`. |
| `CLOUDFLARE_ENABLED` | Must be `true`/`1` — the Workers AI lane is dark until explicitly switched on. |
| `CLOUDFLARE_ACCOUNT_ID` + `CLOUDFLARE_API_TOKEN` | Both required when the lane is enabled; otherwise the provider reports misconfigured and is never called. |
| `GEMINI_API_KEY` | Sent as the `x-goog-api-key` header (never in the URL). |
| `TRAINS_ON_INPUT_PROVIDERS` | Comma list, default `zai`. These providers only serve requests explicitly tagged non-confidential. |

### Routing and fallback

`FALLBACK_CHAIN` is executed, not decorative. Entries are `provider` or
`provider/model`, e.g.:

```
DEFAULT_PROVIDER=groq
FALLBACK_CHAIN=groq/groq/compound,gemini/gemini-2.0-flash
```

When `openai/gpt-oss-120b` hits Groq's daily token cap (429 with
"tokens per day (TPD)"), the next chain entry serves the request. Every
response carries a `synthia` receipt — the provider/model that actually
answered, the data class, the cost, and the full attempt log — so a fallback
is never a silent downgrade. Non-retryable errors (400/401/403) stop the
chain instead of spraying a bad request across every provider.

### Safety rules

- Providers on the `TRAINS_ON_INPUT_PROVIDERS` list (default: `zai`) refuse
  standard traffic with `403 restricted_provider`. Callers must tag the
  request with header `x-data-class: non-confidential` or body
  `metadata.data_class: "non-confidential"` to use them, and they are skipped
  in fallback chains for standard traffic.
- The daily budget middleware (`DAILY_BUDGET_USD`) halts completion traffic
  with `429 budget_exceeded`; `/health`, `/synthia/*` and `/admin` stay
  reachable so a halted gateway can still be inspected.
- Verified free-tier models (Groq free tier, Cloudflare Workers AI free
  allocation, OpenRouter `:free` variants, local Ollama) cost $0 in the spend
  ledger. Unknown models get a conservative non-zero estimate — a paid model
  can never slip through as free.
- Provider health is tracked per provider: repeated failures open a circuit
  and the provider is skipped until the reset window elapses. State is
  mirrored to the `provider_status` table and visible at `/synthia/providers`
  and `/synthia/status`.
