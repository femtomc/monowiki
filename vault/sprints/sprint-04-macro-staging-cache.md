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
- Expand-time executor for `!staged` blocks with sandbox hooks.
- Durability tiers (volatile/session/durable/static) wired into queries.
- Content-addressable caching for expand outputs (per section) with hit-rate metrics.

## Workstreams (parallel)
- **Stream A (Macros/Staging):** Enforestation, macro application rules, diagnostics, tests/goldens.
- **Stream B (Caching/Durability):** Hashing strategy, cache store, integration with incremental engine, benchmarks for edit-to-render latency.

## Risks / Out of Scope
- No render-time live code yet.
- Security sandboxing for staged execution remains minimal (stub ok).

## Exit Criteria
- Macro+staging pipeline produces deterministic, typed `Content`; tests cover hygiene and staging errors.
- Incremental engine shows cache hits and honors durability tiers in profiling runs.
