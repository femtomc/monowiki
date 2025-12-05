---
title: markdown::typst_math::TypstMathRenderer::render_math
description: null
summary: Replace `InlineMath` / `DisplayMath` events with raw HTML containing SVG.
date: null
type: doc
tags:
- rust
- api
- kind:method
- module:markdown::typst_math::TypstMathRenderer
draft: false
updated: null
slug: markdown-typst-math-typstmathrenderer-render-math
permalink: null
aliases:
- markdown::typst_math::TypstMathRenderer::render_math
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

# markdown::typst_math::TypstMathRenderer::render_math

**Kind:** Method

**Source:** [monowiki-core/src/markdown/typst_math.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/markdown/typst_math.rs#L49)

```rust
pub fn render_math(&self, events: Vec<Event<'static>>, preamble: Option<&str>, note_slug: Option<&str>, source_path: Option<&str>,) ->(Vec<Event<'static>>, Vec<Diagnostic>)
```

Replace `InlineMath` / `DisplayMath` events with raw HTML containing SVG.

## Reference source: [monowiki-core/src/markdown/typst_math.rs L49–L99](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/markdown/typst_math.rs#L49)

```rust
    /// Replace `InlineMath` / `DisplayMath` events with raw HTML containing SVG.
    pub fn render_math(
        &self,
        events: Vec<Event<'static>>,
        preamble: Option<&str>,
        note_slug: Option<&str>,
        source_path: Option<&str>,
    ) -> (Vec<Event<'static>>, Vec<Diagnostic>) {
        let mut diagnostics = Vec::new();

        let events = events
            .into_iter()
            .map(|event| match event {
                Event::InlineMath(math) => match self.render_math_block(&math, false, preamble) {
                    Ok(html) => Event::InlineHtml(CowStr::Boxed(html.into_boxed_str())),
                    Err(err) => {
                        warn!("Typst inline math failed: {err}");
                        diagnostics.push(Diagnostic {
                            code: "math.render_failed".to_string(),
                            message: format!("Math rendering failed: {err}"),
                            severity: DiagnosticSeverity::Warning,
                            note_slug: note_slug.map(|s| s.to_string()),
                            source_path: source_path.map(|s| s.to_string()),
                            context: Some(math.to_string()),
                            anchor: None,
                        });
                        Event::InlineMath(math)
                    }
                },
                Event::DisplayMath(math) => match self.render_math_block(&math, true, preamble) {
                    Ok(html) => Event::Html(CowStr::Boxed(html.into_boxed_str())),
                    Err(err) => {
                        warn!("Typst display math failed: {err}");
                        diagnostics.push(Diagnostic {
                            code: "math.render_failed".to_string(),
                            message: format!("Math rendering failed: {err}"),
                            severity: DiagnosticSeverity::Warning,
                            note_slug: note_slug.map(|s| s.to_string()),
                            source_path: source_path.map(|s| s.to_string()),
                            context: Some(math.to_string()),
                            anchor: None,
                        });
                        Event::DisplayMath(math)
                    }
                },
                other => other,
            })
            .collect();

        (events, diagnostics)
    }
```
