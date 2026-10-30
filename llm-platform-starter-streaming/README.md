# Local-First LLM Intelligence Platform

A Rust-first, CLI-driven foundation for a local LLM platform. This repository implements a foundation vertical slice from the supplied specifications: explicit SQLite migrations, provider/model registration, model listing, a loopback-only HTTP API, and reusable persistent job-queue primitives with leases and crash recovery.

> **Scope notice:** this is a working foundation, not the complete platform. It can call registered OpenAI-compatible inference endpoints, including SSE streaming, and persist basic chat sessions. It does not load GGUF/model weights directly, authenticate HTTP requests, execute agents, implement an intelligent router, or train models. The persistent queue includes CLI inspection, queued-job cancellation, lease recovery, and a worker for chat_completion jobs. Model registration remains metadata-only and never executes files.

## Requirements

- Rust stable (edition 2021)
- SQLite support is included via SQLx; no separate SQLite CLI is required
- Python 3.11+ only for the portable migration/schema test in `tests/schema_test.py`

The supplied `specs/` files remain in the repository as source requirements.

## Quick start

```bash
# 1. Compile the Rust executable
cargo build

# 2. Create/update the local schema (migrations are explicit, never automatic at startup)
cargo run -- db migrate

# 3. Register a provider and one model record
cargo run -- providers add local-llama --provider-type local
cargo run -- models register \
  --provider local-llama \
  --external-id my-model-q4 \
  --display-name "My local model" \
  --local-path "$HOME/models/my-model.gguf" \
  --context-window 8192

# 4. Inspect database-backed metadata
cargo run -- models list
cargo run -- db status

# 5. Run a completion through an OpenAI-compatible endpoint
cargo run -- chat my-model-q4 "Explain quantization in one paragraph"

# Restructure a prompt for inspection (does not call a model)
cargo run -- prompt optimize --mode advanced "Compare these three deployment options"

# Optional bearer token, if your endpoint requires one
# export LLM_PLATFORM_API_KEY=...

# 6. Start the HTTP API on loopback only
cargo run -- serve --bind 127.0.0.1:8787
```

Then open these endpoints locally:

- `GET http://127.0.0.1:8787/health/live`
- `GET http://127.0.0.1:8787/health/ready`
- `GET http://127.0.0.1:8787/v1/models`
- `POST http://127.0.0.1:8787/v1/chat/completions`

Provider/model registrations write transactional audit records. Provider endpoint URLs are contacted only when you explicitly invoke `chat` or `POST /v1/chat/completions`. The inference adapter supports non-streaming and SSE-streaming OpenAI-compatible chat-completion endpoints, including cancellation requests for active streams; it does not load GGUF files directly.

The API refuses non-loopback bind addresses until authentication and authorization have been implemented. Do not expose it through a reverse proxy or port-forwarding workaround in this phase.

### Configuration

The database URL can be set with `LLM_PLATFORM_DATABASE_URL` or overridden with `--database-url`. The default is `sqlite://llm-platform.db` in the current working directory. Logging uses `RUST_LOG`, defaulting to `info`.

SQLite foreign keys are enabled on connections and WAL journal mode is requested. Keep the database on a local filesystem and restrict file permissions. Back up the database using SQLite's online backup facilities or after stopping the application; do not treat copying only the main file while WAL is active as a guaranteed consistent backup.

## Test the schema

```bash
python tests/schema_test.py
cargo test
```

The Python test executes the migration using the SQLite library shipped with Python and verifies core constraints. It does **not** replace Rust compilation and SQLx integration tests. Build/test claims should be based on commands actually run in the target environment.

## Current commands

```text
llm-platform db migrate
llm-platform db status
llm-platform providers add <name> [--provider-type <type>] [--endpoint <url>]
llm-platform providers list
llm-platform models register --provider <name> --external-id <id> --display-name <name> [--local-path <path>] [--context-window <tokens>]
llm-platform models list
llm-platform chat <model> <prompt> [--system <instruction>]
llm-platform prompt optimize <prompt> [--mode conservative|advanced]
llm-platform serve [--bind 127.0.0.1:8787]
```

## Repository map

```text
src/                    CLI, SQLite connection/migrations, registry, inference adapter, prompt optimizer, HTTP API
migrations/sqlite/      Immutable, versioned SQLite SQL migrations
specs/                  Original user-supplied specifications
 tests/                  Portable migration/schema checks
 docs/                   Architecture, security boundaries, implementation status
```

## Current implementation sequence

1. **Foundation (in progress):** SQLite migration, persistent provider/model registry, health and model-listing API.
2. **Inference (in progress):** OpenAI-compatible non-streaming and SSE-streaming chat adapter, CLI/API path and HTTP-level stream cancellation; direct local runtime and capability detection remain outstanding.
3. **Orchestration:** queue workers, explainable router, parallel model runs, prompt optimization.
4. **Agents and automation:** bounded plans, action-level approval, tools, scheduling and crash recovery.
5. **Training and integrations:** datasets, fine-tuning adapters, provider integrations.
6. **Multi-user/GUI:** authentication, authorization, server mode and optional Flutter client.
7. **Production readiness:** reproducible installs, backups, recovery and security review.

See [implementation status](docs/STATUS.md), [architecture](docs/ARCHITECTURE.md), and [security boundaries](docs/SECURITY.md).


### Multi-turn chat

The HTTP chat endpoint forwards the complete ordered `messages` history to the provider. For example:

```json
{
  "model": "llama3.2",
  "messages": [
    {"role": "system", "content": "Answer concisely."},
    {"role": "user", "content": "What is quantization?"},
    {"role": "assistant", "content": "It reduces model weight precision."},
    {"role": "user", "content": "What is the trade-off?"}
  ]
}
```

The chat-completions API supports SSE when `stream: true`; its response header `X-LLM-Platform-Request-Id` can be used with `POST /v1/requests/{id}/cancel`. Persistent conversation-message responses are still non-streaming.


### Persistent chat API

After registering a model and starting `llm-platform serve`, create a conversation with `POST /v1/conversations`, then send messages to `POST /v1/conversations/{id}/messages`. The platform stores the ordered conversation history in SQLite and includes prior user/assistant turns when calling the selected model. See `docs/API.md` for request examples and current limitations.


## Persistent jobs

Enqueue a completion using a registered model, then run a one-shot or continuous worker:

```bash
cargo run -- jobs enqueue chat_completion '{"model":"my-model-q4","messages":[{"role":"user","content":"Explain quantization."}]}' --priority 5 --idempotency-key demo-request
cargo run -- jobs list --status queued
cargo run -- jobs worker --once
cargo run -- jobs inspect JOB_ID
```

Run `cargo run -- jobs worker` for a polling worker that exits on Ctrl+C. The initial handler supports `chat_completion`; payloads contain a model ID and ordered messages. Results are stored as JSON in the job result reference. Use a lease longer than the provider's 180-second request timeout; the default is 240 seconds. Only queued jobs can be cancelled.
