---
title: "Nota-style math in monowiki"
date: "2025-01-22"
type: doc
tags: [math, typst, nota]
typst_preamble: |
  #let typeOf(e, t) = $ Γ ⊢ #e : #t $
  #let step(e1, e2) = $ #e1 → #e2 $
---

This note shows how to get a Nota-like experience while staying in plain Markdown. Two building blocks matter:

- A per-note Typst preamble (set in frontmatter) so you can define macros and reuse them across inline and display math.
- A lightweight `@Block` syntax for callouts like definitions and theorems without leaving Markdown.

## Typst preamble in frontmatter

Add any Typst setup under `typst_preamble` in frontmatter. It is prepended to every math expression in the note and cached per-preable:

```yaml
typst_preamble: |
  #let typeOf(e, t) = $ Γ ⊢ #e : #t $
  #let step(e1, e2) = $ #e1 → #e2 $
```

Use your macros in math as usual (pass math content as `$...$` so Typst knows the arguments are math expressions):

Inline: `$#step($e$, $e′$)$` renders a single-step reduction arrow.

Display:

$$
#typeOf($e$, $τ$)
$$

Use `$$...$$` for display math (shown above). Single `$...$` works for inline math like $x + y = z$ or $alpha arrow.r beta$ (Typst uses words for Greek letters and symbols).

## Nota-like blocks

Start a paragraph with `@Kind[label]{Title}: ...` to wrap it in a styled block. The label becomes the `id` (defaults to a slug of the title). Wikilinks, math, and typst macros still work inside the body.

Example definition:

@Definition[label=typing]{Typing judgment}: Every well-formed expression obeys $$ #typeOf($e$, $τ$) $$.

Example theorem:

@Theorem{Progress}: If $$ #typeOf($e$, $τ$) $$ then either `e` is a value or there exists `e′` such that $$ #step($e$, $e′$) $$. See [the typing definition](#typing).

You can link to these blocks with standard anchors: `[Typing](#typing)`, `[Progress](#progress)`, etc.
