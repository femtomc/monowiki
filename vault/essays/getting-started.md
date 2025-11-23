---
title: Getting Started with Monowiki
description: Quick start guide for installing and using monowiki
date: 2025-01-22
type: essay
tags:
  - guide
  - quickstart
---

# Getting Started with Monowiki

Monowiki is a minimalist static site generator for Obsidian-style vaults. It turns markdown notes into a fast, monospace-first static site.

## Why Monowiki?

- Works directly on your vault; no metadata database
- Fast Rust build pipeline
- Wikilinks (`[[Page]]`) and sidenotes (`[^sidenote: text]`)
- Pure static HTML/CSS/JS output you can host anywhere
- Monospace-first styling with a small TypeScript bundle

## Installation

### From Source (Requires Rust)

```bash
cargo install --path monowiki-cli
```

## Quick Start

### 1. Initialize a Project

```bash
monowiki init
```

This creates:
- `monowiki.yml` - Configuration file
- `vault/essays/` - Long-form content
- `vault/thoughts/` - Short-form notes
- `vault/drafts/` - Unpublished content

### 2. Write Content

Create a markdown file in `vault/essays/`:

```markdown
---
title: My First Post
date: 2025-01-22
type: essay
tags:
  - rust
  - webdev
---

# My First Post

This is my first post. Wikilinks let me type `[[another note]]` without worrying about paths.

I can also add sidenotes[^sidenote: This is a margin note!] to my content.
```

### 3. Build Your Site

```bash
monowiki build
```

Your site is now in `docs/` - ready to deploy to GitHub Pages, Netlify, or any static host.

### 4. Development Mode

For live rebuilding:

```bash
monowiki dev
```

Opens at `http://localhost:8000` with automatic rebuilds when files change.

## Next Steps

- Learn about [[configuration]] options in `monowiki.yml`
- Explore [[markdown-features]] like wikilinks, sidenotes, and code blocks
- Report issues on [GitHub](https://github.com/femtomc/monowiki/issues)
