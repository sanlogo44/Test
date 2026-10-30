# Security boundaries and operating guidance

## Implemented now

- The API defaults to `127.0.0.1` and rejects a non-loopback bind address because authentication is not yet implemented.
- SQLite foreign keys and basic status/JSON/range checks are enabled in the schema.
- Parameterized SQL queries are used for runtime values.
- Provider commands accept no secret argument. `secret_ref` is reserved for a future secret-management integration; raw keys must not be placed in `configuration_json`.
- Provider endpoints must be absolute HTTP(S) URLs and reject embedded user info, query strings, and fragments.
- Provider/model creation and the matching audit record are committed in the same transaction; audit metadata contains identifiers and names, not credentials or prompts.
- The model registry never contacts providers, downloads, loads, or executes registered model paths.
- API error responses avoid returning internal database error details to clients; server-side errors are logged.

## Not implemented yet

- User authentication, session/token management, and per-resource authorization.
- Secret manager integration and key rotation.
- Attachment upload validation and managed artifact storage.
- Tool manifests, action-bound approvals and agent sandboxing.
- Rate limiting, quotas, request-size limits and network-facing deployment hardening.
- PostgreSQL role separation and multi-user isolation.

## Required practices before network exposure

Do not expose this API beyond loopback. Before enabling remote server binds, implement authentication, authorization on every operation, TLS termination guidance, request and concurrency limits, audit logging for sensitive actions, secret references backed by an actual secret store, and security integration tests. Agent tools must authorize every invocation at execution time; approval must be bound to the exact action and expire.

Keep local database files private. Never commit credentials, user prompts or model artifacts. Backups can contain conversations and metadata and must receive equivalent access controls.
