# Development status

## Completed in the initial slice

- [x] Retained the two supplied prompts under `specs/`.
- [x] Defined a Rust CLI and module boundaries.
- [x] Added a versioned SQLite foundation migration.
- [x] Added explicit migration and migration-status commands.
- [x] Added provider registration/listing, endpoint validation, and transactional audit events.
- [x] Added model metadata registration/listing with no execution side effects and transactional audit events.
- [x] Added live/readiness and database-backed model-listing HTTP endpoints.
- [x] Added the first real inference path through an OpenAI-compatible chat-completions endpoint, callable from the CLI and HTTP API.
- [x] Added a deterministic conservative/advanced prompt structurer that preserves and prints the original request for review.
- [x] Added connection/request timeouts and environment-only bearer-token support; provider errors do not echo response bodies.
- [x] Refused non-loopback API binds until authentication exists.
- [x] Added schema checks and architecture/security documentation.
- [x] Added persistent job queue primitives: idempotent enqueue, priority-based atomic claims, worker leases, attempt tracking, completion/failure handling, dead-lettering, and expired-lease recovery.
- [x] Authored Rust unit/API tests for registry round-trips, endpoint validation, and route behavior (execution remains unverified here).

## Not yet verified in this environment

The execution environment used to create this starter does not currently have `rustc`, `cargo`, or `rustfmt` installed. Rust compilation, formatting checks, `cargo test`, and end-to-end HTTP tests have therefore **not been run** here. A `Cargo.lock` also could not be generated in this environment. The portable Python/SQLite schema test is separate and does not imply that the Rust code compiles.

## Next work items

- [ ] Run `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test` in a Rust-enabled environment.
- [ ] Execute existing Rust tests and extend coverage to readiness/database failures and server-bind restrictions.
- [ ] Wire job queue primitives into a worker process and CLI/API lifecycle; add operational metrics and configurable backoff.
- [ ] Implement a direct local inference adapter and model capability detection; the current adapter requires a running OpenAI-compatible endpoint.
- [ ] Add tests for inference request/response and streaming/cancellation handling using a local mock server.
- [ ] Persist prompt profiles and optimization decisions, and add user approval/rejection flow before automatically applying optimized prompts.
- [ ] Add configuration versioning and structured error codes.
- [ ] Add PostgreSQL migrations only when the persistence abstraction and tests can verify equivalent behavior.
- [ ] Add authentication/authorization before any remote or multi-user operation.


## Continued implementation

- The OpenAI-compatible adapter now accepts a complete ordered message history, rather than reducing API requests to only the latest user turn and first system message.
- Chat request validation checks for a model, messages, supported roles, non-empty message content, and at least one user message before contacting a provider.
- The HTTP API no longer returns raw provider/internal error text to clients for inference failures; detailed diagnostics remain in server logs.
- Ordered multi-turn history and persistent conversations have been added. SSE streaming and HTTP-level cancellation now exist for `/v1/chat/completions`; conversation persistence still uses non-streaming inference. Provider integration tests and Rust compile/test verification remain outstanding.


## Latest continuation

- Added persistent conversation CRUD/read support using the existing SQLite `conversations` and `messages` tables.
- Added a message-send route that loads ordered history, invokes the selected OpenAI-compatible model, and persists both user and assistant messages.
- Added basic conversation title/content validation and API tests for creation, listing, and invalid titles.
- Remaining: streaming persistence in conversation sessions, auth/ownership, concurrent message sequencing, retry/idempotency keys, provider integration tests, and Rust compile/test verification.


## Streaming and cancellation slice

- [x] Added `stream: true` to `/v1/chat/completions` for Server-Sent Events using OpenAI-compatible upstream streams.
- [x] Normalized upstream delta chunks to the common `chat.completion.chunk` response shape.
- [x] Return an `X-LLM-Platform-Request-Id` response header and added `POST /v1/requests/{id}/cancel`.
- [x] Detects when the client drops its response body and closes the upstream response; cancellation is cooperative at the HTTP layer.
- [x] Added Rust unit tests for chunk translation/message validation and an API test for unknown cancellation IDs (not executable in this environment).
- [ ] Streaming persistence for `/v1/conversations/{id}/messages`, integration tests with a mock SSE provider, and Rust compile/test verification remain outstanding.


## Persistent worker continuation

- [x] Added bounded job listing with validated status filters and limits.
- [x] Added cancellation for queued jobs; running jobs remain owned by their worker lease.
- [x] Added a real chat_completion worker handler using the registered OpenAI-compatible inference adapter.
- [x] Added CLI commands to enqueue, list, inspect, cancel, recover expired leases, and run one-shot or continuous workers.
- [x] Worker failures are logged, retried within the configured attempt limit, and dead-lettered when exhausted.
- [x] Added tests for queue listing, cancellation and failure of unsupported job types.
- [ ] Rust formatting, Clippy, Rust unit tests, and schema tests must pass in CI before this slice is verified.
- [ ] Retry backoff and lease renewal remain required before long-running or expensive production use.

The worker accepts a JSON payload with a model string and messages array. The result is persisted as JSON in result_reference. Provider request timeout is 180 seconds; default worker lease is 240 seconds.
