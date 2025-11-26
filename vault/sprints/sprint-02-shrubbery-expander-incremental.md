---
title: "Sprint 02 – Shrubbery + Expander and Incremental Skeleton"
draft: true
---

# Sprint 02 – Shrubbery + Expander and Incremental Skeleton

**Depends on:** Sprint 01

## Goals
- Parse Djot-inspired Markdown + `!` escapes into shrubbery and deterministically expand to `Content`.
- Stand up a Salsa-style incremental engine skeleton for read/expand stages.

## Scope & Deliverables
- Shrubbery parser (token tree, grouping, no macros yet).
- Expander: shrubbery → `Content` handling `!staged` blocks (basic expressions/loops; no user macros/hygiene), fenced code metadata, attributes, block/inline rules.
- Feature flag in CLI to render via new pipeline.
- Incremental engine crate: query registration, dependency tracking, hashing, early cutoff; queries wired for `source_text → parse_shrubbery → expand_to_content`.

## Workstreams (parallel)
- **Stream A (Parsing/Expand):** Grammar coverage, fixtures/goldens, error reporting.
- **Stream B (Incremental):** Engine API, invalidation strategy, simple metrics on cache hits/misses.

## Risks / Out of Scope
- No macros/hygiene yet.
- Layout and cross-doc resolution remain simple; no perf tuning beyond early cutoff.

## Exit Criteria
- CLI can opt into shrubbery/expander path and produce matching HTML for baseline docs.
- Incremental engine passes unit tests and can wrap the new pipeline stages.
