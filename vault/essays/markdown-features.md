---
title: Markdown Features
description: Special markdown syntax supported by monowiki
date: 2025-01-22
type: essay
tags:
  - markdown
  - syntax
  - features
---

# Markdown Features

Monowiki extends standard markdown with a few focused additions.

## Wikilinks

Link between notes using double brackets:

```markdown
[[Page Title]]
[[page-slug|Custom Link Text]]
```

Monowiki resolves these to the right pages, supports aliases/permalinks, and records backlinks.

## Sidenotes

Add margin notes using the sidenote syntax:

```markdown
This is the main text[^sidenote: This appears in the margin!] that flows normally.
```

Sidenotes are:
- Automatically numbered
- Positioned in the page margin
- Mobile-responsive (inline on small screens)
- Perfect for citations, asides, and commentary

Example: This documentation[^sidenote: Built with monowiki itself!] demonstrates the features.

## Standard Markdown

All standard markdown syntax is supported:

### Headings

```markdown
# H1
## H2
### H3
```

### Emphasis

```markdown
*italic* or _italic_
**bold** or __bold__
~~strikethrough~~
```

### Lists

```markdown
- Unordered list
- Another item
  - Nested item

1. Ordered list
2. Second item
```

### Code Blocks

With syntax highlighting:

```rust
fn main() {
    println!("Hello, monowiki!");
}
```

```python
def greet():
    print("Hello, world!")
```

### Tables

| Feature | Status |
|---------|--------|
| Wikilinks | ✓ |
| Sidenotes | ✓ |
| Code highlighting | ✓ |
| Math | ✓ |

## Math Support

Math is rendered to inline SVG at build time with [Typst](https://typst.app), so there’s no client-side JavaScript required. Use standard `$ ... $` for inline math and `$$ ... $$` for display equations.

Inline example: $E = m c^2$.

Display example:

$$
a^2 + b^2 = c^2
$$

## Frontmatter

Every page should have YAML frontmatter:

```yaml
---
title: Page Title
description: SEO description
date: 2025-01-22
type: essay        # or "thought"
tags:
  - tag1
  - tag2
draft: false       # set true to skip publishing
permalink: /custom/path  # optional
---
```

## What's Next?

- See [[getting-started]] for installation and setup
- Configure your site with [[configuration]] options
