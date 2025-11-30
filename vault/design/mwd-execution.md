# Monowiki Markdown (MWD)

Goals: keep Markdown flexible, add lightweight affordances (links/notes/math/citations), and stay friendly to agents without forcing a rigid spec or embedded language.

## MWD Syntax (quick reference)
- Frontmatter (optional): `title`, `description`, `summary`, `date`, `updated`, `type`, `tags`, `draft`, `slug`, `permalink`, `aliases`, `typst_preamble`, `bibliography`.
- Wikilinks: `[[slug]]` or `[[slug#section]]`; aliases resolve via `aliases` frontmatter and slugified titles. Backlinks graph is built from these.
- Nota blocks: paragraph starting with `@Kind[label]{Title}: body` (label optional, defaults to slugified title). Renders a styled block.
- Sidenotes: `[^sidenote: text]` produces numbered sidenotes; suppressed inside code blocks.
- Math: `$...$`, `$$...$$`, `\(...\)`, `\[...\]`; Typst renders to inline/block SVG; `typst_preamble` frontmatter appends custom macros/preamble.
- Citations: `[@key]` inline; bibliography from `monowiki.yml` and per-note `bibliography` list. Renders numbered inline cites and a references list.
- Code fences: standard Markdown fences; syntax highlighting via syntect.
- TOC: headings auto-slugified and a TOC is injected when headings exist.
- Directory tree macro: `{{directory_tree}}` expands to a pre-rendered tree of notes (based on source paths in the vault).
- Links/base URLs: `base_url` normalized with leading/trailing slash; permalink overrides output path.
- Drafts: `type: draft` or `draft: true` exclude from renders/search/exports.

## Collab/editor considerations
- Storage: Loro document (`LoroNoteDoc`) with blocks, per-block text, marks, comments, frontmatter. Snapshots and updates are broadcast over a channel.
- Comments/marks: CRDT supports marks with anchors and comments; expose as review threads/annotations in the editor.
- Preview: render cache can render a single note with backlinks/Typs t/citations using the cached site index.
- Watch/build: `monowiki dev` watches the vault and rebuilds; `monowiki watch` streams change events (now rooted at `docs/`).

## Optional execution
- The current build does not ship MRL; markdown is rendered statically. Live cells/runtimes are deferred. If/when execution returns, it should remain opt-in with explicit capability gating.

## Known gaps / follow-ups
- Incremental caching: `check_dependencies_changed` always returns true; fix dependency tracking so edits don’t recompute everything and preview stays fast.
- UX surfacing: document these rules in the editor (cheat sheet), and expose backlinks/search/related in the editor sidebar.
- Escape/edge cases: clarify interactions with code fences/inline code (math and sidenote transforms already skip code), and document how to escape `[[...]]` or `@Kind{}` when needed.
- Agent ergonomics: keep APIs stable (`/api/search`, `/api/note`, `/api/graph`, `monowiki note/search/graph/export`) and add structured endpoints for comments/marks so agents can annotate docs without schema lock-in.
