---
title: "Sprint 03 – Typing, Show/Set, and CRDT Strategy"
draft: true
---

# Sprint 03 – Typing, Show/Set, and CRDT Strategy

**Depends on:** Sprint 02

## Goals
- Enforce structural typing for `Content` and introduce the safe show/set rule engine.
- Pick the CRDT stack (target: Loro with MovableTree/Fugue/Peritext-style layers) and map it onto `OperationalDoc`.

## Scope & Deliverables
- Type checks for `Inline`/`Block` invariants, diagnostics with source spans.
- Show/set rule engine with filters and transforms; tests/goldens.
  - Implement `it` binding semantics (see MRL spec §3.3 for type rules)
  - Ensure show rules are type-preserving (`K -> K` constraint)
  - Support attribute-based selectors for custom elements (e.g., `span.where(class == "my-macro")`)
- CRDT migration plan: block IDs, mark anchors, fractional ordering, storage schema, and projected assertions layout.
- Updated projection layer plan from operational → semantic with new CRDT model.

## Reference Documentation
- See `vault/design/mrl.md` §3.3 for detailed type rules on `it` binding in show rules
- See `vault/design/mrl.md` §9.4-9.6 for show/set rule examples
- See `vault/design/mrl.md` FAQ "How do selectors for custom elements work?"

## Workstreams (parallel)
- **Stream A (Typing/Show-Set):** Implement validator, friendly errors, ensure expand output obeys invariants.
- **Stream B (CRDT Strategy):** Evaluate data structures, design migration path from Yrs, document dataspace views that map to CRDT state.

## Risks / Out of Scope
- No user-facing macro DSL yet.
- No live migration executed; this sprint produces the plan and interfaces.

## Exit Criteria
- Show/set rules usable in expand pipeline with tests.
- Agreed CRDT model and documented migration steps/impacts on storage and projection.
