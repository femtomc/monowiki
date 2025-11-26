---
title: "Sprint 01 – Semantic Core & Operational Boundary"
draft: true
---

# Sprint 01 – Semantic Core & Operational Boundary

**Depends on:** `vault/design.md`

## Goals
- Establish the typed `Content` model with value semantics.
- Carve out an operational→semantic boundary so storage choices can change independently of rendering/expansion.

## Scope & Deliverables
- New crate/module defining `Content`, `Block`, `Inline`, `ContentKind`, `Code<K>`, and `+` composition.
- Minimal HTML renderer over `Content` to keep CLI output working.
- `OperationalDoc` trait wrapping current Yrs document (sections, text spans, marks placeholders) plus stub projection into `Content`.

## Workstreams (parallel)
- **Stream A (Semantic Core):** Type definitions, constructors, structural invariants, HTML renderer, tests/goldens.
- **Stream B (Operational Abstraction):** Trait over current Yrs doc, stub projection hook to semantic pipeline, keep existing behavior intact.

## Risks / Out of Scope
- No macros/staging logic yet.
- No CRDT migration; still on existing Yrs layout.
- No incremental queries beyond current full rebuild.

## Exit Criteria
- CLI can render via `Content` renderer behind a flag or adapter.
- Operational layer is behind an interface so later CRDT/tree changes do not touch semantic/render code.
