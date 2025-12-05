---
title: models::Diagnostic
description: null
summary: Structured diagnostic record for verification
date: null
type: doc
tags:
- rust
- api
- kind:struct
- module:models
draft: false
updated: null
slug: models-diagnostic
permalink: null
aliases:
- models::Diagnostic
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

# models::Diagnostic

**Kind:** Struct

**Source:** [monowiki-core/src/models.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/models.rs#L379)

```rust
pub struct Diagnostic
```

Structured diagnostic record for verification

## Reference source: [monowiki-core/src/models.rs L379–L401](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/models.rs#L379)

```rust
/// Structured diagnostic record for verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Machine-readable code (e.g., "link.unresolved")
    pub code: String,
    /// Human-friendly message
    pub message: String,
    /// Severity of the issue
    pub severity: DiagnosticSeverity,
    /// Note slug associated with this diagnostic (if any)
    #[serde(default)]
    pub note_slug: Option<String>,
    /// Source path of the note within the vault (if known)
    #[serde(default)]
    pub source_path: Option<String>,
    /// Additional context (e.g., target slug, citation key)
    #[serde(default)]
    pub context: Option<String>,

    /// Optional related anchor/id
    #[serde(default)]
    pub anchor: Option<String>,
}
```
