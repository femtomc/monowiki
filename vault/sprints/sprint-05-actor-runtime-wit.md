---
title: "Sprint 05 – Actor Runtime Skeleton and WIT/Capabilities"
draft: true
---

# Sprint 05 – Actor Runtime Skeleton and WIT/Capabilities

**Depends on:** Sprint 04

## Goals
- Stand up an in-process syndicated actor runtime with dataspaces and supervision.
- Draft and validate WIT interfaces plus capability prompts for plugins.

## Scope & Deliverables
- Dataspace abstraction (`system`, `doc-content/<doc-id>`, `doc-view/<doc-id>`), assertion publish/subscribe, auto-retraction on actor death.
- Actor lifecycle: start/stop, supervision hooks, minimal scheduler.
- WIT interfaces (document-reader/writer, editor-ui, commands, keybindings, selection-manager, http/fs gated) and plugin manifest schema with capability request UX copy.
- Mapping from query outputs to dataspace assertions (outline, diagnostics, etc.) via projection hooks.

## Workstreams (parallel)
- **Stream C (Actors/Dataspaces):** Runtime core, assertion store, supervision tests. (Continuation of runtime track.)
- **Stream D (WIT/Capabilities):** Interface definitions, manifest parsing, capability gating flow, sample stub plugin. (Continuation of extension track.)

## Risks / Out of Scope
- No distributed dataspaces yet (single process is fine).
- No full plugin execution; stubs acceptable for validation.

## Exit Criteria
- Actor runtime can run sample actors and retract assertions on failure.
- WIT definitions and manifest/capability flow reviewed and aligned with dataspace model.
