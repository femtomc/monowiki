---
title: "Agent Brief – Sprints"
draft: true
---

# Agent Brief – Sprints

## Purpose
Guide agents who maintain or execute sprint plans. Sprint docs live in `vault/sprints/` and align with `vault/design.md`.

## How to use the sprint docs
- Each `sprint-XX-*.md` lists goals, deliverables, parallel workstreams, and exit criteria. Keep those sections up to date when scope shifts.
- When work lands early or slips, append a short "Status" note with date and rationale instead of rewriting history.
- If a sprint changes dependency ordering, update both the affected sprint file and downstream files that listed the old dependency.

## Coordination rules
- Keep semantic vs. operational concerns split: semantic (Content/macro/staging) changes should not bake in CRDT/storage details; operational changes should project into semantic via the agreed boundary.
- Plugins and kernels remain actorized and capability-gated; do not let plugin presence affect document semantics.
- When introducing flags for new pipelines, leave the legacy path working until the sprint exit criteria are verified.

## Reporting
- Summaries should include: shipped deliverables, known gaps vs. exit criteria, perf/regression notes, and follow-ups queued for the next sprint.
- Link code changes to sprint goals so later agents can trace intent.
