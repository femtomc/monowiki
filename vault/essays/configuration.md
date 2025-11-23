---
title: Configuration
description: Complete reference for monowiki.yml configuration options
date: 2025-01-22
type: essay
tags:
  - configuration
  - reference
---

# Configuration

Monowiki is configured via `monowiki.yml` in your project root. All paths are resolved relative to the config file location.

## Minimal Configuration

```yaml
site:
  title: "My Site"
  author: "Your Name"
  description: "Site description"
  url: "https://yoursite.com"

paths:
  vault: "vault"
  output: "docs"
```

## Complete Configuration

Here's every option with defaults and descriptions:

```yaml
site:
  title: "My Site"
  author: "Your Name"
  description: "A knowledge base"
  url: "https://example.com"
  intro: "Optional intro paragraph for homepage"  # optional

paths:
  vault: "vault"              # Source markdown directory
  output: "docs"              # Build output directory
  templates: null             # Custom template directory (optional)
  theme: null                 # Custom theme directory (optional)

server:
  port: 8000                  # Dev server port

# Rust version features
base_url: "/"                 # Base URL for deployed site
enable_rss: true              # Generate RSS feed
enable_sitemap: true          # Generate sitemap.xml
enable_backlinks: true        # Show backlinks on pages
ignore_patterns: []           # Files/dirs to ignore
```

## Path Resolution

All paths in `paths:` are resolved relative to the config file:

```yaml
paths:
  vault: "vault"           # ./vault
  vault: "../vault"        # ../vault
  vault: "/abs/path"       # /abs/path (absolute)
```

## Site Metadata

### title
Site name, shown in header and page titles.

### author
Your name, used in footer and meta tags.

### description
SEO description for the homepage.

### url
Full URL where site will be deployed. Used for:
- RSS feed links
- Sitemap URLs
- Social meta tags

### intro
Optional introduction paragraph on homepage.

## Paths Configuration

### vault
Directory containing your markdown files. Monowiki scans:
- `vault/essays/` - Long-form content
- `vault/thoughts/` - Short notes
- `vault/drafts/` - Unpublished (skipped in build)
- Any `.md` file anywhere in vault

### output
Where built HTML/CSS/JS files are written. Common choices:
- `docs/` - For GitHub Pages
- `public/` - For Netlify/Vercel
- `build/` - Generic output

### templates
Override built-in templates by pointing to a directory with:
- `index.html` - Homepage template (Askama syntax)
- `post.html` - Individual page template (Askama syntax)

### theme
Override built-in CSS/JS by pointing to a directory with:
- `css/` - CSS files
- `js/` - JavaScript files

Files are copied as-is to output. You can also point `theme_overrides` at a folder to layer changes on top of the default theme.

## Server Options

```yaml
server:
  port: 8000
```

Port for `monowiki dev` command.

## Advanced Options

### base_url
If deploying to a subdirectory (e.g., `https://example.com/blog/`):

```yaml
base_url: "/blog/"
```

All links will be prefixed with this path.

### ignore_patterns
Skip files/directories during build:

```yaml
ignore_patterns:
  - ".obsidian"
  - "*.draft.md"
  - "private/"
```

## Next Steps

- See [[getting-started]] to set up your first site
- Learn about [[markdown-features]] like wikilinks, sidenotes, and math
- Adjust the default theme by copying `static/` and pointing `paths.theme` at your copy
