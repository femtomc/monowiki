# Monowiki

Static site generator for markdown vaults. Monospace theme, wikilinks, backlinks graph, Typst math, sidenotes, search.

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/femtomc/monowiki/main/install.sh | sh
```

Or `cargo install --git https://github.com/femtomc/monowiki monowiki-cli`

## Usage

```bash
monowiki init         # scaffold vault/ and monowiki.yml
monowiki dev          # serve at localhost:8000 with live reload
monowiki build        # write static site to docs/
monowiki verify       # advisory vault health report (use --json for agents)
monowiki changes      # summarize git changes since a ref
monowiki comment add  # create an annotation file in vault/comments/
monowiki status       # changes + comments in one call
# Tip: use "-" for comment body to read from stdin; default author from git config
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

## GitHub Pages

```bash
monowiki github-pages
git add .github && git commit -m "Add pages workflow" && git push
```

Enable Pages in repo settings → select "GitHub Actions" as source.

## Config

`monowiki.yml`:

```yaml
site:
  title: My Site
  author: Name
  url: https://example.github.io/repo
paths:
  vault: vault
  output: docs
base_url: /repo/  # for GitHub Pages subpaths
```

## CLI for agents

JSON output for LLM tooling:

```bash
monowiki search "query" --json --with-links
monowiki note <slug> --format json
monowiki graph neighbors --slug <slug> --json
monowiki export sections --format jsonl  # for embeddings
monowiki verify --json  # diagnostics: unresolved links, citations, math, aliases
monowiki changes --since HEAD~1 --json --with-sections
monowiki comment list --slug note-slug --json
monowiki comment add --slug note-slug --anchor section-id --quote "..." --body "text"
monowiki status --since HEAD~1 --comment-status open --json
```

Dev server exposes `/api/search`, `/api/note/<slug>`, `/api/graph/<slug>`.

## License

MIT
