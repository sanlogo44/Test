# Architecture (Phase 1)

## Decisions

- **Modular monolith:** one Rust executable with separate CLI, database, registry and HTTP modules. Separate processes are deferred until isolation, GPU scheduling or operational scaling gives a concrete reason.
- **CLI-first:** the CLI is the current operator interface. The HTTP API exposes only small shared contracts; a future GUI must use those contracts rather than link to internal modules.
- **SQLite first:** SQLite makes the first local-first workflow reproducible without a server dependency. PostgreSQL support is planned behind the persistence boundary and is not implemented yet.
- **Explicit migrations:** `llm-platform db migrate` applies forward-only SQLx migrations. Normal application startup refuses an uninitialized or failed schema instead of silently migrating it.
- **Provider-neutral metadata:** model/provider records are database-backed, but metadata is not treated as proof that an inference runtime can load a model.
- **Loopback-only API:** all HTTP routes lack authentication in this phase; a non-loopback listener is rejected.

## Dependency graph

```text
CLI (clap) ───────┬──── Database (SQLx / SQLite)
                  ├──── Registry (provider/model metadata)
                  └──── HTTP API (Axum)
                           ├──── health/live
                           ├──── health/ready ── Database
                           └──── v1/models ───── Registry ── Database

Next boundary: unified inference trait -> runtime adapters -> router/queue -> chat and agents
```

## Data and migration strategy

`migrations/sqlite/` is the schema source of truth. Migration files are chronological and must not be edited after release. Add a new migration for changes. Foreign-key enforcement is configured on each SQLx SQLite connection. JSON-bearing fields are validated with SQLite's `json_valid` check. Identifiers are UUID strings; timestamps are UTC ISO-8601 strings. Model runtime state and live capability must later be reconciled with the real runtime rather than inferred from rows alone.

The initial migration covers users/roles, providers/models, conversations/messages/model runs, persistent jobs/job attempts, and audit events. Agent definitions, approvals, benchmarks, training data, artifacts, detailed scheduling, and PostgreSQL-specific queue claiming are later increments.

## Model adapter direction

The next inference slice should define a provider-independent Rust trait around capabilities, health checks, load/unload, completion, streaming, cancellation, embeddings and structured output. Each backend should explicitly advertise supported operations. Routing begins with deterministic constraints and recorded explanations; an optional small routing model must never be a single point of failure. llama.cpp is a candidate first local backend, but the adapter contract should not embed runtime-specific behavior in the CLI or database repository.


## Persistent worker boundary

Enqueue, claim, completion, failure, cancellation and lease recovery are explicit SQLite operations. The initial worker handler executes registered-model OpenAI-compatible chat completions and stores a JSON result reference. Unknown job types fail visibly and consume only bounded attempts. Worker IDs are unique by default; completion and failure are ownership-checked against the current lease. Lease renewal and retry backoff remain follow-up work.
