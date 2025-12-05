---
title: models::DiagnosticSeverity
description: null
summary: Severity level for diagnostics emitted during build/verification
date: null
type: doc
tags:
- rust
- api
- kind:enum
- module:models
draft: false
updated: null
slug: models-diagnosticseverity
permalink: null
aliases:
- models::DiagnosticSeverity
typst_preamble: null
bibliography: []
target_slug: null
target_anchor: null
git_ref: null
quote: null
author: null
status: null
parent_id: null
---

# models::DiagnosticSeverity

**Kind:** Enum

**Source:** [monowiki-core/src/models.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/models.rs#L370)

```rust
pub enum DiagnosticSeverity
```

Severity level for diagnostics emitted during build/verification

## Reference source: [monowiki-core/src/models.rs L370–L377](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/models.rs#L370)

```rust
/// Severity level for diagnostics emitted during build/verification
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}
```
