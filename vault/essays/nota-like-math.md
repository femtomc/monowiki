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

## More advanced examples

Here are some examples inspired by programming language theory papers:

### Type checking with contexts

A typical type checking judgment uses multiple contexts:

$$
Sigma; Delta; Gamma tack.r e : tau arrow.r.double Gamma'
$$

Where $Sigma$ is the global function context, $Delta$ contains type variables, and $Gamma$ tracks local bindings.

### Inference rules

Rules can have multiple premises stacked vertically using a fraction for the inference line:

$$
frac(
  Gamma tack.r e_1 : tau_1 quad Gamma\, x : tau_1 tack.r e_2 : tau_2,
  Gamma tack.r "let" x = e_1 "in" e_2 : tau_2
)
$$

### Set operations and mappings

Dependency contexts map memory locations to sets of dependencies:

$$
cal(D) : { overline(ell |-> { overline(ell') }) }
$$

After an assignment $x := e$ at location $ell_3$, we update conflicts:

$$
cal(D)' = cal(D)[x |-> cal(D)(x) union {ell_3}]
$$

### Test bad syntax

This will fail to show error formatting:

$$
frac(test, test, test)
$$
