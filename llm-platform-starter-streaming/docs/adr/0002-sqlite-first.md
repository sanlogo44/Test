# ADR 0002: SQLite first, PostgreSQL later

- Status: Accepted
- Date: 2026-10-29

## Context

The first usable workflow should work on one local machine without requiring a database server. Multi-user server operation needs PostgreSQL behavior and operational testing that is not part of the first slice.

## Decision

Implement SQLx with SQLite and forward-only SQL migrations first. Keep persistence calls behind a small module and document SQLite-specific queue coordination. Add PostgreSQL as a separately tested backend when server mode is implemented.

## Consequences

- A newcomer can initialize the project without provisioning a service.
- SQLite's single-writer behavior must shape job claiming.
- PostgreSQL compatibility must not be claimed until migrations, transactions and integration tests exist for it.
