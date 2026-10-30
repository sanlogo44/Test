# ADR 0001: Start as a modular monolith

- Status: Accepted
- Date: 2026-10-29

## Context

The platform spans inference, routing, model management, jobs, agents and APIs. Splitting these into services immediately would add deployment and operational complexity before boundaries and resource isolation needs are validated.

## Decision

Start with a single Rust executable and explicit internal modules. Separate processes only where GPU scheduling, sandboxing, multi-user isolation or scaling demonstrates a measurable need.

## Consequences

- One reproducible local install and simpler end-to-end tests.
- Module boundaries must remain explicit to avoid a monolith of tightly coupled code.
- Runtime isolation will be introduced deliberately when required by security or workload management.
