# Monowiki

Static site generator for markdown vaults, evolving toward a typed, staged document calculus with CRDT-backed collaboration and an actor runtime for plugins and kernels.

**Current features:** Monospace theme, wikilinks, backlinks graph, Typst math, sidenotes, citations, search.

**In development:** Typed `Content` model, hygienic macro system, incremental computation, real-time collaboration.

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/femtomc/monowiki/main/install.sh | sh
```

Or `cargo install --git https://github.com/femtomc/monowiki monowiki-cli`

## Quick Start

```bash
monowiki init         # scaffold vault/ and monowiki.yml
monowiki dev          # serve at localhost:8000 with live reload
monowiki build        # write static site to docs/
monowiki github-pages # generate GitHub Actions workflow
```

Notes go in `vault/`. Frontmatter is optional:

```yaml
---
title: My Post
date: 2024-08-19
tags: [rust, notes]
---
```

Link between notes with `[[wikilinks]]`. The backlinks graph and search index are built automatically.

## Features

- **Wikilinks & Backlinks**: `[[Page]]` syntax with automatic backlink tracking
- **Sidenotes**: Margin notes via `[^sidenote: text]` syntax
- **Math**: Typst-rendered math (no client-side JS) with `$...$` and `$$...$$`
- **Citations**: BibTeX support with `[@key]` syntax and automatic reference lists
- **Syntax Highlighting**: Code blocks with language-aware highlighting
- **Search**: Client-side full-text search

## GitHub Pages

```bash
monowiki github-pages
git add .github && git commit -m "Add pages workflow" && git push
```

Enable Pages in repo settings → select "GitHub Actions" as source.

## Configuration

`monowiki.yml`:

```yaml
site:
  title: My Site
  author: Name
  url: https://example.github.io/repo
paths:
  vault: vault
  output: docs
base_url: /repo/          # for GitHub Pages subpaths
bibliography:
  - vault/references.bib  # BibTeX files for citations
enable_rss: true
enable_sitemap: true
enable_backlinks: true
```

See `monowiki.yml.example` for all options.

## CLI for Agents

JSON output for LLM tooling:

```bash
monowiki search "query" --json --with-links
monowiki note <slug> --format json
monowiki graph neighbors --slug <slug> --json
monowiki export sections --format jsonl  # for embeddings
```

Dev server exposes `/api/search`, `/api/note/<slug>`, `/api/graph/<slug>`.

## Architecture

Monowiki is a Rust workspace with these crates:

| Crate | Purpose |
|-------|---------|
| `monowiki-cli` | CLI commands and dev server |
| `monowiki-core` | Markdown parsing, frontmatter, build pipeline |
| `monowiki-render` | HTML generation, templates |
| `monowiki-types` | Shared type definitions |
| `monowiki-collab` | Real-time collaboration (Yrs-based) |
| `monowiki-incremental` | Incremental computation engine |
| `monowiki-mrl` | Monowiki Reflective Language (staged macros) |
| `monowiki-runtime` | Actor runtime for plugins/kernels |
| `monowiki-adapters` | External service integrations |

## Design Documents

The project's architecture is documented in `vault/design/`:

- **`design.md`**: Core architecture—typed staging, CRDT collaboration, actor model, incremental computation
- **`mrl.md`**: MRL (Monowiki Reflective Language) specification—grammar, type system, macro hygiene

Sprint planning lives in `vault/sprints/`.

## License

MIT
