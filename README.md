# Monowiki

Site generator from Obsidian vaults. 
Fast builds, monospace theme, hover previews, sidenotes, Typst math, backlinks, search, and optional ORCID-powered paper lists.

## Why? 

- Obsidian-native: keep writing in `vault/`, ship static HTML/JS/CSS.
- Wiki-friendly: [[links]], link previews, backlinks graph, section-aware search.
- Batteries included: Typst math, syntax highlighting, sidenotes, RSS/sitemap.
- Themeable: override templates/theme or drop Berkeley Mono in `theme/css/fonts/`.
- Subpath-safe: `base_url` support for GitHub Pages and similar hosts.

## Install

**Option 1: Download pre-built binary** (recommended)

Download the latest release for your platform from [GitHub Releases](https://github.com/femtomc/monowiki/releases):

```bash
# macOS (Apple Silicon)
curl -L https://github.com/femtomc/monowiki/releases/latest/download/monowiki-macos-aarch64.tar.gz | tar xz
sudo mv monowiki /usr/local/bin/

# macOS (Intel)
curl -L https://github.com/femtomc/monowiki/releases/latest/download/monowiki-macos-x86_64.tar.gz | tar xz
sudo mv monowiki /usr/local/bin/

# Linux (x86_64)
curl -L https://github.com/femtomc/monowiki/releases/latest/download/monowiki-linux-x86_64.tar.gz | tar xz
sudo mv monowiki /usr/local/bin/

# Windows (x86_64)
# Download monowiki-windows-x86_64.zip and extract monowiki.exe
```

**Option 2: Install from source** (requires Rust toolchain)

```bash
cargo install --git https://github.com/femtomc/monowiki monowiki-cli
```

## Quick start

```bash
monowiki init            # writes monowiki.yml and vault/{essays,thoughts,drafts,templates}
monowiki dev             # build + serve at http://localhost:8000 with live rebuilds
monowiki build           # generate static site into docs/ (or configured output)
monowiki github-pages    # set up GitHub Actions for automatic deployment
monowiki search "rust" --json --limit 5 --with-links  # agent-friendly search output
```

Write markdown under `vault/essays/` or `vault/thoughts/`. Each note can use YAML frontmatter:

```yaml
title: "My Post"
date: "2024-08-19"
type: essay        # essay | thought | draft | doc
tags: [rust, wiki]
summary: "One-liner for previews"
draft: false
```

## Configuration

`monowiki.yml` (created by `init`) governs paths and metadata:

```yaml
site:
  title: "My Research Blog"
  author: "Your Name"
  description: "Thoughts on X and Y"
  url: "https://example.com"   # canonical site URL
paths:
  vault: "vault"               # where your notes live
  output: "docs"               # where HTML is written
  templates: null              # optional override templates dir
  theme: null                  # optional override theme dir
base_url: "/"                  # set to "/blog/" for subpaths
ignore_patterns: []            # regexes relative to vault/
enable_rss: true
enable_sitemap: true
enable_backlinks: true
orcid:
  enabled: false
  id: "0000-0000-0000-0000"    # optional ORCID to list papers
server:
  port: 8000
```

## Agent/automation features
- Structured search: `monowiki search "<query>" --json --limit 10 --types essay,thought --tags rust --with-links`. JSON is wrapped in `{schema_version:"2024-11-llm-v1", kind:"search.results", data:{...}}` with slugs, outgoing, backlinks.
- Single-note fetch: `monowiki note <slug> --format json --with-links` (enveloped JSON with frontmatter, rendered HTML, raw markdown, toc, outgoing/backlinks).
- Graph queries: `monowiki graph neighbors --slug <slug> --depth 2 --direction both --json` and `monowiki graph path --from a --to b --json` return the same envelope the dev server uses.
- Embedding/export: `monowiki export sections --format jsonl --with-links` emits section-level chunks ready for vector stores.
- Watch mode: `monowiki watch` streams JSON change events from `vault/`.
- `monowiki init` now drops `vault/AGENT.md` summarizing these workflows for LLM agents.
- Cached site index: builds/dev write `docs/.site_index.json`; `note`, `graph`, and `export` reuse it to avoid a rebuild when querying.

### Dev server JSON API
- Start with `monowiki dev` then call:
  - `/api/search?q=term&limit=10&types=essay,thought&tags=rust`
  - `/api/note/<slug>`
  - `/api/graph/<slug>?depth=2&direction=both`
  - `/api/graph/path?from=a&to=b&max_depth=5`

## Deploying to GitHub Pages

```bash
monowiki github-pages
```

This command:
- Creates `.github/workflows/deploy-pages.yml` workflow
- Auto-detects your repository from git remote
- Configures automatic deployment on push to main
- Sets correct `base_url` for your repository

Then:
1. Commit and push: `git add .github && git commit -m "Add GH Pages" && git push`
2. Enable Pages at: `https://github.com/your-username/your-repo/settings/pages`
3. Select "GitHub Actions" as the source
4. Your site will be live at: `https://your-username.github.io/your-repo/`

For custom domains or subpaths, manually set `base_url` in `monowiki.yml`.

## Theming and assets
- Default theme is bundled; override with your own `templates/` or `theme/` dirs.
- Drop `Berkeley Mono Variable.otf` into `theme/css/fonts/` to use the monospace default.
- Static assets in `static/` are copied as-is to the output.

## License

MIT; see `LICENSE`.
