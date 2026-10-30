# ADR 0003: Keep the unauthenticated API on loopback

- Status: Accepted
- Date: 2026-10-29

## Context

The first HTTP routes do not have authentication or per-resource authorization. A configurable bind address could otherwise expose unauthenticated model inventory and future functionality over a network.

## Decision

Default to loopback and reject non-loopback bind addresses in the application until authentication, authorization and network-facing security tests are implemented.

## Consequences

- Safe local experimentation is possible without implying server readiness.
- Network testing requires a future explicit security milestone, not a documentation-only warning.
