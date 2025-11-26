---
title: "Sprint 06 – Plugin Integration and Live Code Execution"
draft: true
---

# Sprint 06 – Plugin Integration and Live Code Execution

**Depends on:** Sprint 05

## Goals
- Integrate WIT-based plugins into the actor runtime with capability enforcement.
- Add render-time live code execution (`!live`) via kernel actors.

## Scope & Deliverables
- WASM plugin host wired to WIT interfaces, capability gating, and per-plugin resource limits.
- Sample plugins (spellcheck/outline) using document-reader/editor-ui APIs; decorations/diagnostics projected via `doc-view`.
- Kernel actor contract: `EvalRequest`/`EvalResult` assertions, timeout handling, sandbox for JS/WASM kernels.
- Execution graph for reactive cells; recomputation invalidates render-time outputs without syncing computed values.

## Workstreams (parallel)
- **Stream C (Runtime Integration):** Hook actors to collab server + incremental outputs; propagate selection/presence/diagnostics. (Continuation of runtime track.)
- **Stream D (Plugins/Kernels):** Implement host bindings, sample plugins, kernel sandbox with resource/timeouts. (Continuation of extension track.)

## Risks / Out of Scope
- Distributed dataspaces and server authority remain future work.
- Performance tuning beyond correctness and basic latency targets is deferred.

## Exit Criteria
- Plugins can be enabled/disabled, publish decorations/diagnostics, and are isolated via capabilities.
- Live code cells execute through kernel actors with timeouts and do not sync outputs; render updates re-evaluate correctly after edits/merges.
