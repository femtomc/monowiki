# Agent Brief for monowiki

## Purpose
Quick orientation for coding/ops agents working in this repo. Source of truth for architecture: `vault/design.md`. Active sprint plan: `vault/sprints/`.

## What this project is
- Static site generator evolving into a typed, staged document calculus with incremental queries, CRDT-backed operational state, and an actor runtime for plugins/kernels.
- Key crates: `monowiki-core` (markdown/frontmatter/build), `monowiki-render` (HTML/templates), `monowiki-cli` (commands/dev server), `monowiki-collab` (Yrs-based collab server, to migrate toward MovableTree/Fugue/Peritext/Loro).

## Repo habits & guardrails
- Keep ASCII unless file already uses Unicode; add comments only when clarifying non-obvious code.
- Prefer `rg` for search; use `apply_patch` for single-file edits.
- Do **not** revert user changes or run destructive git commands. Respect sandbox/approval settings.
- Keep CLI output concise; summarize command results instead of pasting long logs.

## Build/test quickstart
- Common commands: `cargo test`, `cargo test -p monowiki-core`, `cargo fmt`, `cargo clippy`.
- Static build: `monowiki build` (uses `monowiki.yml`), dev server: `monowiki dev`, collab server: `monowiki collab`.
- When adding new pipelines, keep legacy paths working behind flags until migrated.

## Architecture touchpoints
- Semantic model (`Content`, staging, macros) and operational model (CRDT/tree/text/marks) are deliberately decoupled; honor that boundary.
- Incremental engine should expose early-cutoff and durability tiers; CRDT changes invalidate queries, not the other way around.
- Plugins/kernels run as actors with capability-gated WIT interfaces; document semantics must not depend on plugins.

## When in doubt
- Cross-check decisions against `vault/design.md`.
- Align changes with the nearest sprint doc in `vault/sprints/`; note any deviations.
