# Phase 1 HTTP API

The server binds to loopback only. These routes are unauthenticated and must not be exposed to a network.

| Method | Path | Purpose |
|---|---|---|
| GET | `/health/live` | Process-level liveness check |
| GET | `/health/ready` | Verifies the database responds to `SELECT 1` |
| GET | `/v1/models` | Lists registered models belonging to enabled providers |
| POST | `/v1/chat/completions` | Chat completion through a registered OpenAI-compatible provider; supports SSE streaming |
| POST | `/v1/requests/{id}/cancel` | Requests cancellation of an active streaming completion |

`/v1/models` returns an array of objects containing `id`, `display_name`, `external_model_id`, `provider_name`, `status`, and nullable `context_window`. The `registered` status means only that metadata is in the database; it does not mean a model is loaded or currently available.


## Chat completions

The request body follows the common OpenAI chat-completions shape, with `model` set to a registered model external ID or display name and `messages` containing the ordered conversation history. A non-streaming success uses the `chat.completion` response shape. Set `stream` to `true` to receive Server-Sent Events (SSE) with `chat.completion.chunk` payloads.

Example:

```bash
curl http://127.0.0.1:8787/v1/chat/completions \
  -H 'Content-Type: application/json' \
  -d '{"model":"my-model-q4","messages":[{"role":"user","content":"Say hello."}]}'
```

Configure a provider endpoint as an OpenAI-compatible base URL such as `http://127.0.0.1:11434/v1`; the adapter appends `/chat/completions`. If the provider requires a bearer token, set `LLM_PLATFORM_API_KEY` in the process environment. This single environment variable is intentionally not stored in SQLite. Do not use this single-key mechanism for multi-user deployments.


### Chat conversation history

`POST /v1/chat/completions` accepts an ordered `messages` array. The platform forwards the full sequence to the selected OpenAI-compatible provider, including prior assistant turns. Supported message roles in this phase are `system`, `user`, `assistant`, and `tool`. At least one user message is required, and message contents must be non-empty.

### Streaming and cancellation

Set `"stream": true` to stream SSE events:

```bash
curl -N -D /tmp/llm-platform-stream-headers.txt \
  http://127.0.0.1:8787/v1/chat/completions \
  -H 'Content-Type: application/json' \
  -d '{"model":"llama3.2","stream":true,"messages":[{"role":"user","content":"Explain quantization."}]}'
```

The response includes `X-LLM-Platform-Request-Id`. While the stream is active, another terminal can request cancellation:

```bash
REQUEST_ID=$(awk 'BEGIN{IGNORECASE=1} /^X-LLM-Platform-Request-Id:/ {gsub("\r", "", $2); print $2}' /tmp/llm-platform-stream-headers.txt)
curl -X POST "http://127.0.0.1:8787/v1/requests/$REQUEST_ID/cancel"
```

The streaming adapter translates provider `data:` messages into OpenAI-style `chat.completion.chunk` events and ends with `data: [DONE]` where possible. The request ID is valid only while the stream is active; unknown or completed IDs return `404`. Disconnecting the consuming HTTP client also drops the upstream stream. Cancellation is cooperative at the HTTP streaming boundary; it does not guarantee that a remote provider immediately stops internal compute. Streaming applies to `/v1/chat/completions`; the SQLite conversation-message route remains non-streaming in this iteration. Tool calls are not orchestrated by the platform yet.


## Persistent conversations

Conversations are stored in SQLite and are intended for the local single-user development phase. These routes have no authentication and must only be exposed on loopback.

| Method | Path | Purpose |
|---|---|---|
| GET | `/v1/conversations` | List the most recent 100 non-archived conversations |
| POST | `/v1/conversations` | Create a conversation; optional `title`, `model`, and `system_prompt` |
| GET | `/v1/conversations/{id}/messages` | Read the ordered message history |
| POST | `/v1/conversations/{id}/messages` | Save a user message, call the selected model with history, and persist the assistant response |

Create a conversation with a registered model:

```bash
curl http://127.0.0.1:8787/v1/conversations \
  -H 'Content-Type: application/json' \
  -d '{"title":"My first chat","model":"llama3.2","system_prompt":"Be concise."}'
```

Append a message (replace `CONVERSATION_ID` with the returned ID):

```bash
curl http://127.0.0.1:8787/v1/conversations/CONVERSATION_ID/messages \
  -H 'Content-Type: application/json' \
  -d '{"content":"Explain quantization."}'
```

The message endpoint currently performs a non-streaming inference call. It persists the user message before calling the provider; if the provider fails, the user message remains saved and the API returns `502`. Retrying the request manually will create another user message. Authentication, per-user ownership checks, concurrent-write coordination, token accounting, and streaming persistence remain unimplemented.
