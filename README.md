# Local-First LLM Intelligence Platform

The updated Rust starter project lives in [`llm-platform-starter-streaming/`](llm-platform-starter-streaming/).

This branch extends the supplied starter with:

- CLI commands for persistent jobs: enqueue, list, inspect, cancel, recover and worker.
- A worker handler for real `chat_completion` jobs through a registered OpenAI-compatible provider.
- Bounded retries with exponential backoff and lease ownership checks.
- A worker lease guard that exceeds the provider's 180-second request timeout.
- A root GitHub Actions workflow for Rust format, Clippy, Rust tests and SQLite schema tests.

## Quick start

See [the project README](llm-platform-starter-streaming/README.md) for setup and configuration. The API remains loopback-only because authentication and authorization are not yet implemented.

## Current limitations

This is an incremental foundation, not the complete platform. Direct GGUF inference, routing, agent execution, scheduled tasks, training, authentication, PostgreSQL and Flutter remain future phases. Rust compilation and Rust tests have not been verified in this development environment; the CI workflow is configured to run them.

## Build output

When the CI workflow succeeds, it publishes an artifact named `llm-platform-starter-updated` containing the updated project ZIP.
