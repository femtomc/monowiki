---
title: "Sprint 04 – Macro & Staging Core, Query Caching"
draft: true
---

# Sprint 04 – Macro & Staging Core, Query Caching

**Depends on:** Sprint 03

## Goals
- Introduce hygienic macros with Python-like syntax, `Code<K>` quote/splice, enforestation, and `!staged` execution at expand-time.
- Add durability tiers and content-addressable caches to the incremental engine.

## Scope & Deliverables
- Macro API with hygiene contexts, quote/splice types, precedence resolution.
  - Implement quote/splice semantics (see MRL spec §3.3)
  - Implement cross-stage persistence (CSP) for value capture
  - Ensure CSP captures VALUES not bindings (see FAQ)
- Expand-time executor for `!staged` blocks with sandbox hooks.
- Durability tiers (volatile/session/durable/static) wired into queries.
- Content-addressable caching for expand outputs (per section) with hit-rate metrics.

## Reference Documentation
- See `vault/design/mrl.md` §3.3 for complete quote/splice semantics and CSP rules
- See `vault/design/mrl.md` §8.1 for expand-time stdlib reference
- See `vault/design/mrl.md` §9.1-9.12 for comprehensive macro examples
- See `vault/design/mrl.md` FAQ "What's the difference between !staged and !live?"

## Workstreams (parallel)
- **Stream A (Macros/Staging):** Enforestation, macro application rules, diagnostics, tests/goldens.
- **Stream B (Caching/Durability):** Hashing strategy, cache store, integration with incremental engine, benchmarks for edit-to-render latency.

## Risks / Out of Scope
- No render-time live code yet.
- Security sandboxing for staged execution remains minimal (stub ok).

## Exit Criteria
- Macro+staging pipeline produces deterministic, typed `Content`; tests cover hygiene and staging errors.
- Incremental engine shows cache hits and honors durability tiers in profiling runs.
