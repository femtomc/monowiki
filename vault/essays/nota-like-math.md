---
title: "Nota-style math in monowiki"
date: "2025-01-22"
type: doc
tags: [math, mathjax, nota]
bibliography:
  - vault/references/nota.bib
---

This note shows how to get a Nota-like experience while staying in plain Markdown (inspired by the Nota language [@nota]). Two building blocks matter:

- LaTeX math rendered client-side with MathJax, supporting macros via `\newcommand`.
- A lightweight `@Block` syntax for callouts like definitions and theorems without leaving Markdown.

## Math with MathJax

Use `\( ... \)` for inline math and `\[ ... \]` or `$$ ... $$` for display equations. MathJax supports standard LaTeX math syntax.

Inline example: \( E = mc^2 \) or $\alpha \to \beta$.

Display example:

$$
\Gamma \vdash e : \tau
$$

## LaTeX macros

You can define macros using standard LaTeX `\newcommand` syntax in display math blocks:

$$
\newcommand{\typeOf}[2]{\Gamma \vdash #1 : #2}
\newcommand{\step}[2]{#1 \to #2}
$$

Then use them in subsequent math:

$$
\typeOf{e}{\tau}
$$

## Nota-like blocks

Start a paragraph with `@Kind[label]{Title}: ...` to wrap it in a styled block. The label becomes the `id` (defaults to a slug of the title). Wikilinks and math still work inside the body.

Example definition:

@Definition[label=typing]{Typing judgment}: Every well-formed expression obeys $$ \Gamma \vdash e : \tau $$.

Example theorem:

@Theorem{Progress}: If $$ \Gamma \vdash e : \tau $$ then either `e` is a value or there exists `e'` such that $$ e \to e' $$. See [the typing definition](#typing).

You can link to these blocks with standard anchors: `[Typing](#typing)`, `[Progress](#progress)`, etc.

## More advanced examples

Here are some examples inspired by programming language theory papers:

### Type checking with contexts

A typical type checking judgment uses multiple contexts:

$$
\Sigma; \Delta; \Gamma \vdash e : \tau \Rightarrow \Gamma'
$$

Where $\Sigma$ is the global function context, $\Delta$ contains type variables, and $\Gamma$ tracks local bindings.

### Inference rules

Rules can have multiple premises stacked vertically using a fraction for the inference line:

$$
\frac{\Gamma \vdash e_1 : \tau_1 \quad \Gamma, x : \tau_1 \vdash e_2 : \tau_2}{\Gamma \vdash \text{let } x = e_1 \text{ in } e_2 : \tau_2}
$$

### Set operations and mappings

Dependency contexts map memory locations to sets of dependencies:

$$
\mathcal{D} : \{ \overline{\ell \mapsto \{ \overline{\ell'} \}} \}
$$

After an assignment $x := e$ at location $\ell_3$, we update conflicts:

$$
\mathcal{D}' = \mathcal{D}[x \mapsto \mathcal{D}(x) \cup \{\ell_3\}]
$$
