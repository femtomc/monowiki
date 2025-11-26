---
title: "CRDT Migration Plan: Yrs → Loro"
draft: false
---

# CRDT Migration Plan: Yrs → Loro

This document outlines the strategy for migrating monowiki's collaborative editing infrastructure from Yrs (Y.js Rust port) to Loro, a more powerful CRDT system with native support for hierarchical documents, rich text, and formatting marks.

## Executive Summary

**Current State:** Yrs with flat Y.Text for document body
**Target State:** Loro with MovableTree + Fugue + Peritext
**Migration Strategy:** Phased rollout with abstraction layer
**Timeline:** Sprint 03 (abstraction) → Sprint 05 (full cutover)

## Current State Analysis

### Yrs Implementation (as of Sprint 02)

The existing CRDT implementation in `monowiki-collab/src/crdt.rs` uses:

```rust
pub struct NoteDoc {
    awareness: Awareness,
    frontmatter: RwLock<Value>,
    // ...
}
```

**Data Model:**
- Single `Y.Text("body")` field containing full markdown
- Frontmatter stored separately as JSON in `RwLock<Value>`
- No structured tree representation
- No native formatting marks
- Character-level CRDT for text sequences

**Strengths:**
- ✅ Proven Y.js CRDT algorithms
- ✅ Good sync protocol
- ✅ Works well for flat markdown

**Limitations:**
- ❌ No block-level structure (everything is flat text)
- ❌ No native support for rich text marks
- ❌ Difficult to implement block-level operations (move, reorder)
- ❌ No Peritext-style anchor semantics
- ❌ Hard to map to semantic Content tree

## Target State: Loro Architecture

### Data Model

Loro provides three integrated CRDT layers:

```text
┌────────────────────────────────────────────────────────┐
│ Layer 1: MovableTree (Document Structure)             │
│  • Hierarchical tree of sections and blocks           │
│  • Fractional indexing for ordering                   │
│  • Move operations preserve identity                  │
└────────────────────────────────────────────────────────┘
                          │
                          ▼
┌────────────────────────────────────────────────────────┐
│ Layer 2: Fugue (Text Sequences)                       │
│  • One Richtext instance per block                    │
│  • Character-level CRDT with causal ordering          │
│  • Efficient for concurrent text edits                │
└────────────────────────────────────────────────────────┘
                          │
                          ▼
┌────────────────────────────────────────────────────────┐
│ Layer 3: Peritext (Formatting Marks)                  │
│  • Marks attached to character ranges                 │
│  • Before/After anchor semantics                      │
│  • Survives concurrent edits at boundaries            │
│  • Supports overlapping marks                         │
└────────────────────────────────────────────────────────┘
```

### Example Document Structure

```
LoroDoc
├─ MovableTree "structure"
│  ├─ Section "intro" (order: 0.0)
│  │  ├─ Heading "h1" (order: 0.0) → Richtext "Introduction"
│  │  └─ Paragraph "p1" (order: 1.0) → Richtext "This document..."
│  ├─ Section "methods" (order: 1.0)
│  │  ├─ Heading "h2" (order: 0.0) → Richtext "Methods"
│  │  ├─ CodeBlock "code1" (order: 1.0) → Richtext "fn main() {...}"
│  │  └─ Paragraph "p2" (order: 2.0) → Richtext "The code above..."
│  └─ ...
│
└─ Frontmatter Map
   ├─ "title" → "My Document"
   ├─ "author" → "Alice"
   └─ ...
```

### Benefits Over Yrs

1. **Native Block Structure**
   - Can move/reorder blocks atomically
   - Section-based organization matches semantic model
   - Easier projection to Content tree

2. **Rich Text Marks**
   - Peritext-style marks with anchor semantics
   - Marks expand/contract correctly with concurrent edits
   - Support for attributes (links, metadata)

3. **Better Semantics**
   - Operations match user intent (move heading, not "delete + insert text")
   - Conflict-free block reordering
   - Cleaner separation of structure vs. content

4. **Performance**
   - Section-level caching more effective
   - Incremental updates for large documents
   - Efficient sync (only changed subtrees)

## Migration Phases

### Phase 1: Abstraction Layer (Sprint 03) ✅

**Goal:** Introduce OperationalDoc trait without breaking existing code.

**Deliverables:**
- ✅ `src/operational.rs` - OperationalDoc trait definition
- ✅ `src/yrs_adapter.rs` - Wrap existing Yrs code in trait
- ✅ `src/loro/` - Loro implementation (feature-gated, placeholder)
- ✅ `src/projection.rs` - Operational → Semantic projection
- ✅ `src/migration.rs` - Migration utilities
- ✅ This document

**Status:** Complete. Existing Yrs functionality unchanged.

### Phase 2: Loro Implementation (Sprint 04)

**Goal:** Fully implement Loro backend behind feature flag.

**Tasks:**
- [ ] Complete LoroOperationalDoc implementation
  - [ ] MovableTree operations (insert, move, delete blocks)
  - [ ] Richtext operations (insert, delete text)
  - [ ] Peritext mark operations (add, remove marks)
- [ ] Loro sync protocol integration
  - [ ] State encoding/decoding
  - [ ] Update application
  - [ ] Awareness/presence
- [ ] Subscription/observer system
  - [ ] Change notifications
  - [ ] Dataspace projection hooks
- [ ] Test suite
  - [ ] Unit tests for all operations
  - [ ] Concurrent edit scenarios
  - [ ] Sync correctness tests

**Acceptance:**
- All OperationalDoc trait methods implemented
- Tests pass with `--features loro`
- Sync works between Loro clients

### Phase 3: Data Migration Tools (Sprint 04-05)

**Goal:** Tools to migrate existing .ydoc files to Loro format.

**Tasks:**
- [ ] Migration script
  - [ ] Read existing .ydoc files
  - [ ] Parse Y.Text into blocks (heuristic: split on blank lines)
  - [ ] Detect block types (heading levels, code blocks, etc.)
  - [ ] Create Loro MovableTree structure
  - [ ] Populate Richtext for each block
  - [ ] Extract and apply marks (if any)
- [ ] Validation
  - [ ] Round-trip test: markdown → Yrs → export → Loro → markdown
  - [ ] Verify text content preserved
  - [ ] Verify structure inferred correctly
- [ ] Migration report
  - [ ] Blocks migrated
  - [ ] Text length
  - [ ] Warnings (e.g., ambiguous structure)

**Output:** CLI tool `monowiki-collab migrate --from-yrs <slug>`

### Phase 4: Parallel Testing (Sprint 05)

**Goal:** Run both backends in production, validate equivalence.

**Tasks:**
- [ ] Dual-write mode
  - [ ] Write to both Yrs and Loro simultaneously
  - [ ] Compare outputs for consistency
- [ ] Shadow reads
  - [ ] Read from Loro, compare with Yrs
  - [ ] Log discrepancies
- [ ] Monitoring
  - [ ] Track migration success rate
  - [ ] Performance comparison (latency, memory)
- [ ] Gradual rollout
  - [ ] 10% of documents use Loro
  - [ ] 50% of documents use Loro
  - [ ] 100% of documents use Loro

**Acceptance:**
- No data loss or corruption
- Performance meets or exceeds Yrs
- Sync reliability >= 99.9%

### Phase 5: Cutover (Sprint 05-06)

**Goal:** Make Loro the default, deprecate Yrs.

**Tasks:**
- [ ] Change default backend to Loro
- [ ] Migrate all remaining .ydoc files
- [ ] Update documentation
- [ ] Remove Yrs code paths (or keep as legacy feature)
- [ ] Celebrate! 🎉

**Acceptance:**
- New documents always use Loro
- Legacy .ydoc files auto-migrate on access
- Yrs code removed or gated behind `legacy` feature

## Data Model Mapping

### Yrs → Loro: Block Inference

Since Yrs stores flat markdown, we must infer block structure:

**Heuristics:**

1. **Headings:** Lines starting with `#` (1-6 times)
2. **Code blocks:** Fenced with ` ``` ` or `~~~`
3. **Lists:** Lines starting with `-`, `*`, `+`, or `1.`
4. **Blockquotes:** Lines starting with `>`
5. **Paragraphs:** Everything else, split on blank lines

**Example:**

```markdown
# Introduction

This is a paragraph.

## Methods

```python
def foo():
    pass
```
```

→ Loro Structure:

```
Section (root)
├─ Heading (level=1, order=0.0)
│  └─ Richtext "Introduction"
├─ Paragraph (order=1.0)
│  └─ Richtext "This is a paragraph."
├─ Heading (level=2, order=2.0)
│  └─ Richtext "Methods"
└─ CodeBlock (lang="python", order=3.0)
   └─ Richtext "def foo():\n    pass"
```

### Block ID Scheme

**Yrs:** Block IDs are synthetic (`block_0`, `block_1`, ...)

**Loro:** Block IDs are Loro OpIds (unique, causal)

**Migration:** Generate new Loro IDs; no stable mapping needed (blocks are recreated).

### Fractional Indexing

**Purpose:** Order siblings without renumbering.

**Example:**

```
Insert block between 0.0 and 1.0 → 0.5
Insert block between 0.0 and 0.5 → 0.25
```

**Implementation:** Loro provides this natively via MovableTree.

### Mark Anchoring (Peritext)

Marks have **start/end anchors** that determine expansion behavior:

| Anchor | Behavior on concurrent insert |
|--------|------------------------------|
| `Before` | Insert before this boundary expands the mark |
| `After` | Insert after this boundary expands the mark |

**Example:**

```
Mark: [emphasis, start=5 (Before), end=10 (After)]
Text: "Hello world"

Edit: Insert "beautiful " at position 6
      ↓
Result: "Hello beautiful world"
Mark becomes: [5, 16] (expanded to include "beautiful ")
```

**Migration:** Yrs doesn't have native marks, so we default to:
- `start_anchor: Before`
- `end_anchor: After`

### Frontmatter

**Yrs:** Stored separately in `RwLock<Value>`
**Loro:** Use a Loro `Map` container

**Migration:** Serialize JSON → deserialize into Loro Map.

## Storage Schema

### File Layout (Current)

```
vault/
├─ notes/
│  └─ example.md          # Markdown with frontmatter
└─ .collab/
   └─ notes/
      └─ example.ydoc     # Yrs binary state
```

### File Layout (After Migration)

```
vault/
├─ notes/
│  └─ example.md          # Still the human-readable version
└─ .collab/
   └─ notes/
      └─ example.loro     # Loro binary state (replaces .ydoc)
```

**Migration:**
- Read `example.ydoc` (Yrs state)
- Parse and convert to Loro
- Write `example.loro`
- Keep `example.ydoc` as backup during transition
- Delete `.ydoc` after verification

## Dataspace Projections

### Assertions in `doc-content/<doc-id>`

With Loro's structured model, we can expose richer assertions:

**Block-level:**

```rust
BlockInfo {
    node_id: "h1",
    parent_id: "root",
    kind: Heading { level: 1 },
    order: FractionalIndex("0.0"),
    attrs: { id: "introduction" }
}
```

**Text-level (per block):**

```rust
BlockText {
    block_id: "p1",
    text: "This is a paragraph.",
}

BlockChanged {
    block_id: "p1",
    range: (5, 7),
    new_text: "was",
}
```

**Mark-level:**

```rust
MarkInfo {
    block_id: "p1",
    mark_id: "m1",
    type: "emphasis",
    start: CharId("..."),
    end: CharId("..."),
    attrs: { ... }
}
```

### Benefits for Plugins

- **Spellchecker:** Subscribe to `BlockText` for paragraphs only
- **Outline generator:** Subscribe to `BlockInfo` for headings only
- **Diff viewer:** Subscribe to `BlockChanged` for incremental updates

## Testing Strategy

### Unit Tests

- ✅ OperationalDoc trait compliance
- [ ] Loro operations (insert, move, delete)
- [ ] Projection correctness
- [ ] Migration round-trips

### Integration Tests

- [ ] Concurrent edits (2+ clients)
- [ ] Block reordering
- [ ] Mark application during edits
- [ ] Sync convergence

### Migration Tests

- [ ] Yrs → Loro → Markdown (content preserved)
- [ ] Large documents (1000+ blocks)
- [ ] Edge cases (empty blocks, nested lists)

### Performance Benchmarks

- [ ] Edit latency (target: <16ms)
- [ ] Sync overhead (target: <100ms RTT)
- [ ] Memory usage (target: <10MB per doc)

## Risks and Mitigations

### Risk 1: Data Loss During Migration

**Mitigation:**
- Keep .ydoc files as backup during transition
- Dual-write phase validates correctness
- Export to JSON before migration

### Risk 2: Loro Bugs or Missing Features

**Mitigation:**
- Loro is production-ready (used by other projects)
- Feature flag allows rollback to Yrs
- Active upstream community

### Risk 3: Performance Regression

**Mitigation:**
- Benchmark before/after migration
- Loro is designed for large documents
- Section-level caching optimizes reads

### Risk 4: Sync Protocol Incompatibility

**Mitigation:**
- Loro has its own sync protocol
- Clients must upgrade together (version negotiation)
- Staged rollout minimizes impact

## Open Questions

### Q1: How to handle documents mid-edit during migration?

**Options:**
1. Lock documents during migration (downtime)
2. Migrate offline, merge changes after
3. Dual-write until migration complete

**Recommendation:** Option 3 (dual-write) for active documents.

### Q2: Should we support bidirectional sync (Yrs ↔ Loro)?

**Recommendation:** No. One-way migration only. Simpler and safer.

### Q3: What about frontmatter changes?

**Yrs:** Separate RwLock
**Loro:** Loro Map

**Recommendation:** Migrate frontmatter to Loro Map for consistency.

### Q4: How to version .loro files?

**Recommendation:** Use Loro's built-in versioning. Include schema version in metadata.

## Success Metrics

- [ ] All existing documents migrated successfully
- [ ] Zero data loss or corruption
- [ ] Sync latency ≤ Yrs baseline
- [ ] Block operations work correctly (move, reorder)
- [ ] Peritext marks expand correctly on concurrent edits
- [ ] Dataspace projections reflect Loro state accurately
- [ ] Plugin ecosystem works with new assertions

## Timeline

| Phase | Sprint | Duration | Status |
|-------|--------|----------|--------|
| Abstraction | Sprint 03 | 1 week | ✅ Complete |
| Loro Implementation | Sprint 04 | 2 weeks | 🟡 Pending |
| Migration Tools | Sprint 04-05 | 1 week | 🟡 Pending |
| Parallel Testing | Sprint 05 | 1 week | 🟡 Pending |
| Cutover | Sprint 05-06 | 1 week | 🟡 Pending |

**Estimated Completion:** End of Sprint 06

## References

- Loro documentation: https://loro.dev
- Peritext paper: Litt et al. "Peritext: A CRDT for Rich-Text Collaboration"
- MovableTree: Kleppmann et al. "A Highly-Available Move Operation for Replicated Trees"
- Yrs documentation: https://docs.rs/yrs/latest/yrs/

## Appendix: Code Locations

**New files (Sprint 03):**
- `/Users/femtomc/monowiki/monowiki-collab/src/operational.rs` - Trait definition
- `/Users/femtomc/monowiki/monowiki-collab/src/yrs_adapter.rs` - Yrs wrapper
- `/Users/femtomc/monowiki/monowiki-collab/src/loro/` - Loro implementation
- `/Users/femtomc/monowiki/monowiki-collab/src/projection.rs` - Projection layer
- `/Users/femtomc/monowiki/monowiki-collab/src/migration.rs` - Migration utilities

**Existing files:**
- `/Users/femtomc/monowiki/monowiki-collab/src/crdt.rs` - Current Yrs implementation
- `/Users/femtomc/monowiki/monowiki-mrl/src/content.rs` - Semantic Content types

---

**Last Updated:** Sprint 03
**Status:** Abstraction layer complete, ready for Loro implementation
