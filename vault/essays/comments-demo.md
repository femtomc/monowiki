---
title: Comments Demo
summary: Showing how inline comments/annotations work
---

# Comments & Annotations

Monowiki supports comment notes (`type: comment`) that target other notes and anchors. They render inline and surface via APIs and `annotations.json`.

To create a comment:

```bash
monowiki comment add \
  --slug comments-demo \
  --anchor comments-anchors \
  --quote "inline markers" \
  --author "Agent" \
  --tags review,open \
  --body "Please verify the anchor resolution logic."
```

Or pipe the body from stdin by passing `--body -`.

Comments live in `vault/comments/` and are collected during build. They appear below the page content, with status (open/resolved), author, anchor, and optional quote snippet.

## Comment Anchors {#comments-anchors}

Anchors can be:
- A stable section id (from search/export) 
- A heading id
- Or resolved by matching the `quote` text to a section

If resolution fails, the comment remains in `annotations.json` and diagnostics flag unresolved anchors.
