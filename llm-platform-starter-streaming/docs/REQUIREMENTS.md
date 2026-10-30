# Requirements and acceptance criteria

## Product requirements

1. **Local-first ownership:** the core workflow must run on local hardware without mandatory cloud connectivity.
2. **One model contract:** runtimes and provider-specific APIs must sit behind adapters that explicitly report supported operations.
3. **CLI-first workflow:** installation, configuration, model inventory and operational tasks must be available without a GUI.
4. **Explainable orchestration:** route decisions must state the selected model and relevant constraints; measured values must be separate from estimates.
5. **Permission-controlled automation:** agents must be bounded by budgets and permissions, with authorization checked for every tool execution.
6. **Persistent jobs:** work must survive process restart, use bounded retries/leases, and not disappear silently.
7. **Data and secrets safety:** database rows, logs, audit records and backups must not become an accidental secret store.
8. **Replaceable user interfaces:** optional Flutter interfaces use stable APIs rather than internal engine modules.

## Non-functional requirements

- Secure defaults and least privilege.
- Reproducible builds, versioned configuration and forward-only migrations.
- Structured logs, health checks, cancellation, explicit timeout behavior and actionable errors.
- Resource-aware inference with estimates clearly labelled and conservative safety margins.
- Automated unit, integration, migration, recovery and authorization tests.
- Graceful feature degradation when a runtime, GPU backend or optional provider is unavailable.

## Foundation acceptance criteria

- [x] Rust crate and module boundaries are defined.
- [x] Schema migration creates the foundation tables, constraints and indexes.
- [x] Schema supports users/roles, providers/models, conversations/messages, model runs, jobs/attempts and audit events.
- [x] Migration is explicit and migration status is inspectable.
- [x] CLI can register/list providers and register/list model metadata.
- [x] HTTP liveness/readiness and database-backed model-listing routes are specified and implemented.
- [x] Initial non-streaming inference adapter is available through CLI and OpenAI-shaped HTTP route, subject to an external compatible runtime.
- [x] Deterministic prompt structuring supports conservative and advanced modes while preserving the original request for inspection.
- [x] API refuses non-loopback addresses until authentication is present.
- [x] Portable schema tests run against the actual migration file.
- [x] Authored Rust unit tests for registry round-trips, endpoint validation, liveness, and schema gating.
- [ ] Rust compilation, formatting checks and tests pass in an environment with a Rust toolchain.
- [ ] Run HTTP tests and add readiness/database-failure and server-bind integration tests.

## Future phase gates

| Phase | Acceptance gate |
|---|---|
| Inference | At least one supported local backend performs a real prompt, streams output, cancels work, and reports capabilities/health. Missing runtime or model errors are explicit. |
| Orchestration | Persistent job claiming is transactional; leases recover abandoned work; routing decisions include candidates/reasons and do not depend on a routing LLM. |
| Agents | Tool permissions are checked at execution time; budgets/timeouts/concurrency are enforced; approval is bound to an exact action and expires. |
| Training | Dataset validation, provenance, quotas, checkpoint storage and evaluation outputs have reproducible tests. |
| Multi-user | Authentication and resource authorization cover every route; secrets use a real secret store; database roles follow least privilege. |
| Production | Clean installation, upgrade/restore drills, full tests, operational docs and security review succeed. |


## Persistent queue worker slice

- [x] Queue records can be listed with bounded limits and validated status filters.
- [x] Queued work can be cancelled before a worker claims it.
- [x] Expired worker leases can be recovered explicitly.
- [x] A CLI worker executes one job or polls continuously until interrupted.
- [x] The initial chat_completion handler invokes a registered provider and persists a structured result reference.
- [ ] Retry backoff and lease renewal are required before long-running or expensive jobs are operationally hardened.
