# Designing a Typed, Staged Document Calculus for Multiplayer Technical Writing

Building a document language for a collaborative WYSIWYG editor requires balancing four concerns: **type safety**, **reactive computation**, **macro extensibility**, and **CRDT-native collaboration**. This document provides concrete architectural recommendations for the monowiki project, prioritizing design decisions that compose well over isolated local optima.

A fifth concern runs underneath all of these: **a principled concurrency and sandboxing model** for plugins, agents, code execution, and clients. Monowiki uses a **syndicated actor model runtime** to structure these concerns.

---

## Table of Contents

1. [Core Calculus and Staging](#1-core-calculus-and-staging)
2. [Concrete Syntax](#2-concrete-syntax)
3. [Type System](#3-type-system)
4. [Incremental Computation](#4-incremental-computation)
5. [Macro System](#5-macro-system)
6. [Editor Architecture and Extensibility](#6-editor-architecture-and-extensibility)
7. [CRDT Design and Collaboration](#7-crdt-design-and-collaboration)
8. [System Architecture](#8-system-architecture)
9. [Implementation Roadmap](#9-implementation-roadmap)
10. [Trade-offs and Open Questions](#10-trade-offs-and-open-questions)

---

## 1. Core Calculus and Staging

### 1.1 Documents as First-Class Values

Recent work on document languages (e.g., from the Brown PLT group) shows that they live on a **2D spectrum**: string vs. structured article (document domain) crossed with literal, template-literal, or template-program (construction method). Typst sits at the most powerful corner—**article-domain template programs**—which is the right target for technical writing with embedded computation.

The foundational type is `Content`: a tree of elements with fields, supporting composition via `+` and structural operations. The key idea is **value semantics**: all document data is tree-shaped with no reference cycles. This yields:

* Pure reference-counting GC
* Trivial serialization
* Equational reasoning about document transformations

These properties are essential for both incremental computation and CRDT synchronization.

### 1.2 Two Representations

It is essential to separate two representations:

| Representation  | Purpose                                                    | Structure                           |
| --------------- | ---------------------------------------------------------- | ----------------------------------- |
| **Operational** | Editor manipulation, CRDT sync, fine-grained collaboration | Rope + Peritext marks + MovableTree |
| **Semantic**    | Type checking, macro expansion, layout, rendering          | `Content` values (typed AST)        |

The operational representation is the source of truth for collaboration. The semantic representation is derived deterministically at expand-time.

### 1.3 Three-Phase Staging Model

Adopt a **three-phase model** inspired by Rhombus and MetaOCaml:

| Phase           | What Happens                                                                            | Inputs → Outputs            |
| --------------- | --------------------------------------------------------------------------------------- | --------------------------- |
| **Read-time**   | Parse source into shrubbery (token tree with grouping, deferred fine-grained structure) | Source text → `Shrubbery`   |
| **Expand-time** | Macro expansion, template instantiation, cross-reference resolution, type checking      | `Shrubbery` → `Content`     |
| **Render-time** | Dynamic computation, layout, interactive behavior                                       | `Content` → Rendered output |

**Typed staging guarantees that generated fragments are well-formed, well-typed, and well-scoped by construction.** Errors surface at expand-time rather than render-time.

### 1.4 Staging Annotations in Source

Users must be able to control when code executes:

````markdown
// Render-time: live, reactive computation (JS/WASM)
!live
  slider = ui.slider(0, 100, value=50)
  ui.show(slider.get() ** 2)

// Expand-time: runs once when document is compiled
!staged
  data = load_json("data.json")
  for row in data.rows:
    paragraph(row.summary)

// Quoted code: not executed, just displayed
```plaintext
print("Hello, world!")
```
````

The distinction (with current runtime constraints):

- **Render-time (`!live`)**: Code runs in the browser, responds to user interaction, re-executes on input changes. Runtime is JS/WASM.  
- **Expand-time (`!staged[…]`)**: Code runs during document compilation, output is baked into the `Content` tree.  
- **Quoted**: Code is displayed but not executed.

### 1.5 Execution and Concurrency Model (Syndicated Actors)

The calculus and staging phases describe *what* is computed and *when*. We also need a model for **concurrent entities** around the document: plugins, agents, evaluation backends, and remote clients.

Monowiki adopts a **syndicated actor model** as its execution and concurrency substrate:

- Programs are **actors** running event loops.
- Actors interact via **dataspaces**: shared "virtual locations" where they:
  - Publish and retract **assertions** representing state.
  - Subscribe to patterns over those assertions.
- The primitive is **state replication in dataspaces**; message-passing and pub/sub are derived from assertion changes.
- Security follows **object-capability** principles: capabilities refer to dataspaces or services and can be attenuated so holders can only read/write specific kinds of assertions.

In monowiki:

- Each **plugin**, **agent**, **code execution kernel**, and **connected client** is modeled as an actor.
- Each **document** and **workspace** is associated with one or more **dataspaces**, e.g.:
  - `doc-content/<doc-id>` — assertions reflecting the CRDT state (blocks, text spans, marks).
  - `doc-view/<doc-id>` — selections, presence, diagnostics, decorations, evaluation requests/results.
  - `system` — service discovery, plugin registration, configuration, capability grants.

This yields a clean separation:

- **Documents**: staged, typed values (`Content`) flowing through read/expand/render.  
- **Concurrency and sandboxing**: syndicated actors operating over dataspaces that expose those values and their metadata as assertions. Documents can reflect on themselves (`!staged[...]` over shrubbery/Content) and shell out to kernels/agents via EvalRequest/Result assertions.

---

## 2. Concrete Syntax

### 2.1 Design Principles

Following Djot and MyST, the syntax prioritizes:

1. **Linear-time parsing** with no backtracking  
2. **Local parsing decisions** (no action-at-a-distance from definitions elsewhere)  
3. **Explicit block boundaries** (blank lines required before blocks)  
4. **Uniform extensibility** via attributes and `!` macro invocations  
5. **Single escape hatch** for computation/syntax: `!` (macros, staged code, show/set)

### 2.2 Grammar (EBNF)

```ebnf
(* ===== Document Structure ===== *)

document     = { block } ;

block        = heading
             | paragraph
             | macro_block      (* !name(args)[content] *)
             | fenced_code
             | blockquote
             | list
             | thematic_break
             | blank_line ;

(* ===== Headings ===== *)

heading      = [ attributes ], heading_marker, inline_content, newline ;
heading_marker = { "#" }- ;  (* 1-6 # characters *)

(* ===== Paragraphs ===== *)

paragraph    = [ attributes ], inline_content, { newline, inline_content }, blank_line ;

(* ===== Macro Blocks (bang-invocation) ===== *)

macro_block  = "!", identifier, [ macro_args ], "[", macro_body, "]" ;
macro_args   = "(", { any_char - ")" }, ")" ;
macro_body   = { block | inline_content } ;

(* ===== Fenced Code Blocks ===== *)

fenced_code  = [ attributes ], fence_open, [ lang ], [ code_options ], newline,
               code_content,
               fence_close, newline ;

fence_open   = "```" | "~~~" ;
fence_close  = "```" | "~~~" ;  (* must match open *)
lang         = identifier ;
code_options = { newline, "#|", whitespace, identifier, ":", whitespace, value } ;
code_content = { any_char - fence_close } ;

(* ===== Lists ===== *)

list         = bullet_list | ordered_list ;

bullet_list  = { bullet_item }- ;
bullet_item  = [ attributes ], bullet_marker, whitespace, inline_content, newline,
               [ nested_content ] ;
bullet_marker = "-" | "*" | "+" ;

ordered_list = { ordered_item }- ;
ordered_item = [ attributes ], digit, { digit }, ".", whitespace, inline_content, newline,
               [ nested_content ] ;

nested_content = indent, { block }, dedent ;

(* ===== Blockquotes ===== *)

blockquote   = { ">", whitespace, ( inline_content | block ), newline }- ;

(* ===== Inline Content ===== *)

inline_content = { inline_element } ;

inline_element = text
               | emphasis
               | strong
               | code_span
               | link
               | image
               | role
               | inline_span
               | reference
               | math_inline
               | raw_inline ;

text         = { char - special_char }- ;
special_char = "_" | "*" | "`" | "[" | "]" | "{" | "}" | "@" | "$" | "\" ;

emphasis     = "_", { inline_element - "_" }-, "_" ;
strong       = "*", { inline_element - "*" }-, "*" ;
code_span    = "`", { any_char - "`" }, "`" ;

link         = "[", inline_content, "]", "(", url, [ whitespace, title ], ")" ;
image        = "!", "[", alt_text, "]", "(", url, [ whitespace, title ], ")" ;

(* Roles: inline macros (surface `!name(args)[content]`) *)
role         = "{", role_name, "}", "`", role_content, "`" ;
role_name    = identifier ;
role_content = { any_char - "`" } ;

(* Inline spans with attributes *)
inline_span  = "[", inline_content, "]", attributes ;

(* Cross-references *)
reference    = "@", identifier ;

(* Math *)
math_inline  = "$", { any_char - "$" }, "$" ;

(* Raw inline for specific formats *)
raw_inline   = "`", { any_char - "`" }, "`", "{=", format, "}" ;

(* ===== Attributes ===== *)

attributes   = "{", [ attribute_list ], "}" ;
attribute_list = attribute, { whitespace, attribute } ;
attribute    = id_attr | class_attr | kv_attr ;
id_attr      = "#", identifier ;
class_attr   = ".", identifier ;
kv_attr      = identifier, "=", ( quoted_string | identifier ) ;

(* ===== Primitives ===== *)

identifier   = letter, { letter | digit | "_" | "-" } ;
quoted_string = '"', { any_char - '"' }, '"' ;
url          = { any_char - ")" - whitespace } ;
title        = '"', { any_char - '"' }, '"' ;
alt_text     = { any_char - "]" } ;
value        = { any_char - newline } ;
newline      = "\n" | "\r\n" ;
blank_line   = newline, newline ;
whitespace   = { " " | "\t" }- ;
indent       = (* context-sensitive: increase in indentation *) ;
dedent       = (* context-sensitive: decrease in indentation *) ;
```

### 2.3 Example Document

````markdown
# Introduction {#sec-intro}

This document demonstrates the *monowiki* syntax, which combines
_Djot-style_ inline formatting with MyST-style extensibility.

## Code Execution

Executable code blocks support both expand-time and render-time evaluation:

```{javascript}
#| name: fig-plot
#| caption: A simple plot
#| live: true
const x = [1, 2, 3, 4, 5];
const y = x.map(v => v * v);
ui.show({ x, y });
```

The result is shown in @fig-plot.

!staged[
let contributors = fetch_json("contributors.json")
for person in contributors:
  paragraph(text("Thanks to ") + emph(text(contributors.name)))
]

## Callouts and Figures (macro-based)

!callout(severity: 'warn)[This is a note with a warning class applied.]

!figure(src: "architecture.png", name: "fig-arch", width: "80%", align: "center")[
The system architecture showing the three-phase pipeline.
]

## Inline Elements

- Cross-references: @sec-intro, @fig-arch
- Inline macros: !abbr(title: "HyperText Markup Language")[HTML]
- Math: $E = mc^2$
- Attributed spans: [highlighted text]{.highlight key="value"}
- Raw HTML: `<br>`{=html}

## Custom Macros

!def theorem(title: String, body: Shrubbery) -> Code<Block>:
  quote:
    directive(
      "div",
      attrs = { "class": "theorem-box" },
      body = strong(text("Theorem (") + text(splice(title)) + text("). ")) + splice(expand(body)),
    )

!theorem(title: "Fundamental Theorem")[Every document is a value.]
````

### 2.4 Syntax Disambiguation Rules

To ensure unambiguous, local parsing:

| Situation | Rule |
|-----------|------|
| Emphasis vs. list marker | `_` for emphasis, `-`/`*`/`+` only at line start for lists |
| Strong vs. emphasis | `*strong*` vs. `_emphasis_` (no doubling) |
| Code fence language | Must be single identifier, no spaces |
| Attribute attachment | `{...}` binds to immediately preceding or following element |
| Reference vs. text | `@identifier` is always a reference; literal `@` requires backslash |
| Heading vs. language escape | `# ` at line start is a heading; language-level escapes use `!` prefix (e.g., `!def`, `!show`, `!macro(...)`) |
| Code fence vs. staged/live | Fenced blocks are literal/foreign; `!staged`/`!live` are the only executable forms |
| Content literal vs. link | `[...]` is Markdown in prose; MRL content literals appear only inside `!` forms |

### 2.5 Language Escape Summary

- **Prose/structure**: Markdown-style headings, lists, paragraphs, inline emphasis, spans with `{#id .class key=value}`.
- **Language escape** (`!`): macros (`!def`, `!name(args)[content]`), staged code (`!staged[...]`), styling (`!show`, `!set`), live cells (`!live`). No whitespace after `!`.
- **Code fences**: literal code or foreign JS snippets; not used for syntax extension or live execution.
- **Content literals**: `[...]` inside MRL (within `!` blocks/expressions) is a content literal. Bare `[...]` in prose is Markdown (links unless no `(url)` follows).
- **Attributes vs. args**: Markdown attributes remain for IDs/classes; macro args carry structured data/behavior. Roles are just inline `!macro(...)`.

---

## 3. Type System

### 3.1 Core Types

```text
┌─────────────────────────────────────────────────────────────────┐
│                         Type Hierarchy                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Value                                                          │
│  ├── Content (document content, composable via +)               │
│  │   ├── Block                                                  │
│  │   │   ├── Heading(level: Int, body: Inline)                  │
│  │   │   ├── Paragraph(body: Inline)                            │
│  │   │   ├── CodeBlock(lang: String?, code: String, opts: Map)  │
│  │   │   ├── List(items: Array<ListItem>)                       │
│  │   │   ├── Blockquote(body: Content)                          │
│  │   │   └── ThematicBreak                                      │
│  │   │                                                          │
│  │   └── Inline                                                 │
│  │       ├── Text(value: String)                                │
│  │       ├── Emphasis(body: Inline)                             │
│  │       ├── Strong(body: Inline)                               │
│  │       ├── Code(value: String)                                │
│  │       ├── Link(body: Inline, url: String, title: String?)    │
│  │       ├── Image(alt: String, url: String, title: String?)    │
│  │       ├── Reference(target: String)                          │
│  │       ├── Math(value: String)                                │
│  │       └── Span(body: Inline, attrs: Attributes)              │
│  │                                                              │
│  ├── Primitive                                                  │
│  │   ├── Unit                                                   │
│  │   ├── Bool                                                   │
│  │   ├── Int                                                    │
│  │   ├── Float                                                  │
│  │   ├── String                                                 │
│  │   └── Symbol                                                 │
│  │                                                              │
│  ├── Composite                                                  │
│  │   ├── Array<T>                                               │
│  │   ├── Map<K, V>                                              │
│  │   ├── Tuple<T...>                                            │
│  │   └── Record { field: Type, ... }                            │
│  │                                                              │
│  ├── Function<Args, Return>                                     │
│  │                                                              │
│  └── Staged                                                     │
│      └── Code<K> where K : ContentKind                          │
│          (quoted code that produces Content of kind K)          │
│                                                                 │
│  ContentKind = Block | Inline | Content                         │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 3.2 Type Rules for Content

Content composition is typed to ensure structural validity:

```text
Γ ⊢ e₁ : Inline    Γ ⊢ e₂ : Inline
────────────────────────────────────  (Inline-Compose)
        Γ ⊢ e₁ + e₂ : Inline

Γ ⊢ e₁ : Block    Γ ⊢ e₂ : Block
──────────────────────────────────  (Block-Compose)
       Γ ⊢ e₁ + e₂ : Block

Γ ⊢ e₁ : Content    Γ ⊢ e₂ : Content
──────────────────────────────────────  (Content-Compose)
        Γ ⊢ e₁ + e₂ : Content

        Γ ⊢ e : Inline
───────────────────────────  (Inline-Sub-Content)
       Γ ⊢ e : Content

        Γ ⊢ e : Block
──────────────────────────  (Block-Sub-Content)
       Γ ⊢ e : Content
```

**Invariant**: `Inline` cannot contain `Block`. This is enforced statically:

```text
Γ ⊢ body : Inline
─────────────────────────────────────  (Emphasis-Intro)
Γ ⊢ emphasis(body) : Inline

Γ ⊢ body : Content    (* NOT Inline-only! *)
───────────────────────────────────────────  (Blockquote-Intro)
Γ ⊢ blockquote(body) : Block
```

### 3.3 Type Rules for Staging

Staging uses `Code<K>` to represent quoted code that will produce content of kind `K`:

```text
Γ ⊢ e : K    K <: ContentKind
─────────────────────────────  (Quote)
   Γ ⊢ quote(e) : Code<K>

Γ ⊢ e : Code<K>
────────────────────  (Splice)
Γ ⊢ splice(e) : K

Γ ⊢ e : Code<K>
──────────────────────  (Eval-Expand)
Γ ⊢ eval_expand(e) : K
(* Only valid at expand-time *)
```

**Example**: A macro that generates a heading (surface `!staged` maps to Eval-Expand, surface `!def` maps to `macro(f)`):

```text
!def section_header(title: String, level: Int) -> Code<Block>:
  quote:
    heading(level = splice(level), body = text(splice(title)))

// Usage in document:
!staged
  section_header("Introduction", 1)

// After expansion, this becomes:
// Heading(level: 1, body: Text("Introduction"))
```

### 3.4 Type Rules for Macros

Macros are functions from `Shrubbery` to `Code<K>`:

```text
Γ ⊢ f : (Shrubbery, MacroContext) -> Code<K>
────────────────────────────────────────────────  (Macro-Type)
            Γ ⊢ macro(f) : Macro<K>

Γ ⊢ m : Macro<K>    Γ ⊢ input : Shrubbery
──────────────────────────────────────────────  (Macro-Apply)
         Γ ⊢ expand(m, input) : K
```

**Hygiene constraint**: Macros receive a `MacroContext` that tracks scopes. Identifiers introduced by the macro are marked with the macro's scope and cannot capture user bindings unless explicitly requested.

Surface mapping: `!def f(...) -> Code<K>` corresponds to `macro(f)`; `!name(args)[content]` applies Macro-Apply after enforestation; `quote`/`splice`/`eval_expand` map to Code<K>/Splice/Eval-Expand.

### 3.5 Show/Set Rule Types

Show and set rules have restricted types for safety:

```text
Γ ⊢ selector : Selector<K>    Γ ⊢ props : Properties<K>
──────────────────────────────────────────────────────────  (Set-Rule)
         Γ ⊢ set(selector, props) : SetRule<K>

Γ ⊢ selector : Selector<K>    Γ ⊢ transform : K -> Content
────────────────────────────────────────────────────────────  (Show-Rule)
          Γ ⊢ show(selector, transform) : ShowRule<K>
```

**Restriction**: Show rules transform content; they cannot introduce new syntax or access shrubbery. This makes them safe for end-users.
**Phase**: Show/set run at expand-time only; they cannot execute staged code or shell out to kernels. Render-time concerns stay in JS/WASM code fences.

### 3.6 Runtime Values and Dynamic Typing

Not all values are statically typed. The type system includes an escape hatch:

```text
       Γ ⊢ e : T
─────────────────────────  (Inject-Dynamic)
   Γ ⊢ dynamic(e) : Dyn

Γ ⊢ e : Dyn    (runtime check that e : T)
──────────────────────────────────────────  (Project-Dynamic)
          Γ ⊢ e as T : T
```

**Use cases for `Dyn`**:

* Values from external code execution (JS/WASM, optional kernels).
* User-provided JSON/YAML data.
* Plugin return values.

Runtime type errors at the `Dyn`/static boundary produce clear error messages with source locations.

---

## 4. Incremental Computation

### 4.1 Queries vs Actors

Monowiki has two reactive systems that serve different purposes:

**Queries** are synchronous, pure, memoized functions for transforming document state:

- They implement the document pipeline: source → shrubbery → Content → layout
- They are demand-driven (only compute what's needed for the current viewport)
- They support early cutoff (if output unchanged, skip downstream recomputation)
- They run in the host process and are never distributed
- They have no side effects and no notion of identity or lifecycle

**Actors** are concurrent entities that communicate via assertions in dataspaces:

- They handle I/O, user interaction, plugins, kernels, and network peers
- They are push-based (react to assertion changes)
- They have identity, lifecycle, and can fail independently
- They can be distributed across processes and machines
- They interact only through capability-mediated dataspaces

**The bridge between them:**

1. **CRDT updates invalidate queries.** When the CRDT layer receives an edit (local or remote), it marks the relevant `source_text` queries as stale.

2. **Query outputs can be projected into dataspaces.** When actors need derived information (e.g., document outline, semantic diagnostics, resolved references), the host computes the relevant queries and asserts the results into a dataspace.

3. **Actors never call queries directly.** An actor that needs parsed content subscribes to assertions; the host ensures those assertions reflect current query outputs.

This separation keeps the document pipeline fast and deterministic while allowing flexible, capability-secure coordination for everything else.

### 4.2 Query-Based Architecture

Following Salsa (as used in rust-analyzer), computation is organized as a graph of **queries**:

```rust
// Pseudo-Rust showing query definitions

#[query]
fn source_text(db: &Db, section: SectionId) -> Rope {
    db.crdt_state().get_section_text(section)
}

#[query]
fn parse_shrubbery(db: &Db, section: SectionId) -> Shrubbery {
    let source = source_text(db, section);
    parser::parse_shrubbery(&source)
}

#[query]
fn expand_to_content(db: &Db, section: SectionId) -> Content {
    let shrubbery = parse_shrubbery(db, section);
    let macros = active_macros(db);
    expander::expand(shrubbery, macros)
}

#[query]
fn resolve_references(db: &Db, doc: DocId) -> ResolvedContent {
    let sections = doc_sections(db, doc);
    let contents: Vec<Content> = sections
        .iter()
        .map(|s| expand_to_content(db, *s))
        .collect();
    resolver::resolve(contents)
}

#[query]
fn layout_section(db: &Db, section: SectionId, viewport: Viewport) -> Layout {
    let content = expand_to_content(db, section);
    let styles = active_styles(db);
    layout::compute(content, styles, viewport)
}
```

### 4.3 Early Cutoff

The critical optimization: if a query's output is structurally equal to its previous output, downstream queries are not invalidated.

```text
Edit: "  Hello  " → "Hello"  (whitespace change)
  │
  ▼
source_text(sec1) → changed
  │
  ▼
parse_shrubbery(sec1) → Shrubbery { Paragraph(Text("Hello")) }
                        (structurally equal to previous!)
  │
  ▼
expand_to_content(sec1) → NOT RECOMPUTED (early cutoff)
  │
  ▼
layout_section(sec1) → NOT RECOMPUTED
```

### 4.4 Durability Tiers

Queries are partitioned by expected change frequency:

| Tier         | Examples                                  | Invalidation    |
| ------------ | ----------------------------------------- | --------------- |
| **Volatile** | User content, cursor position, selections | Every edit      |
| **Session**  | Active viewport, UI state                 | User actions    |
| **Durable**  | Theme, macros, configuration              | Explicit reload |
| **Static**   | Built-in functions, core library          | Never           |

Changes to volatile queries skip checking durable queries entirely—a significant optimization for interactive editing.

### 4.5 Content-Addressable Caching

Section-level outputs are stored by content hash:

```rust
struct SectionCache {
    // Hash of (source_hash, macro_version, config_hash) -> Content
    content_cache: HashMap<Hash, Content>,
    
    // Hash of (content_hash, style_hash, viewport_hash) -> Layout
    layout_cache: HashMap<Hash, Layout>,
}
```

This enables:

* Cross-session caching (same content → same output).
* Parallel computation (independent sections computed concurrently).
* Efficient sync (only transfer changed sections).

---

## 5. Macro System

### 5.1 Shrubbery Representation

Source is first parsed into a **shrubbery**—a token tree with structure but deferred grouping:

```text
Input: "f(1 + 2 * 3)"

Shrubbery:
  Group(
    Identifier("f"),
    Parens(
      Sequence(
        Number(1),
        Operator("+"),
        Number(2),
        Operator("*"),
        Number(3)
      )
    )
  )
```

The key property: **operator precedence is not yet resolved**. Macros can participate in precedence parsing via "enforestation."

### 5.2 Macro Definitions (Typst-inspired surface, Python-flavored bodies)

We adopt a Typst-like surface for macros and use `!` as the language escape to avoid heading conflicts. Define with `!def ...:`, invoke with `!name(args)[content]`. Block and inline uses share the same form; the content slot is optional. Macro bodies use Python-like indentation and `quote:` blocks for staged code.

```text
!def callout(severity: Symbol = 'info, body: Shrubbery) -> Code<Block>:
  quote:
    class_map = {
      'note: "callout-note",
      'warn: "callout-warn",
      'error: "callout-error",
    }
    cls = class_map.get(splice(severity), "callout-info")
    directive(
      "div",
      attrs = { "class": cls },
      body = splice(expand(body)),
    )

// Block invocation
!callout(severity: 'warn)[Be careful with this operation.]

// Inline macro (role-style)
!def abbr(title: String, body: Shrubbery) -> Code<Inline>:
  quote:
    span(
      splice(expand(body)),
      attrs = { "title": splice(title) },
    )
 
Use !abbr(title: "HyperText Markup Language")[HTML] inline.

// Self-reflective example (outline)
!staged[
outline = doc.outline()
for h in outline:
  paragraph(f"{h.level}. {h.title}")
]
```

**Surface ↔ calculus mapping**
- `!def f(...) -> Code<K> = ...` defines a `Macro<K>` (Type Rule: Macro-Type).
- `!name(args)[content]` enforces precedence/enforestation, producing a `Shrubbery` applied via Macro-Apply.
- `quote`/`splice`/`eval_expand` correspond to Code<K>, Splice, Eval-Expand rules.

### 5.3 Hygiene

Macros use lexical scoping with explicit scope markers:

```text
!def with_caption(cap: String, body: Shrubbery) -> Code<Block>:
  quote:
    // 'caption' is in the macro's scope, not the user's
    caption = splice(cap)
    figure(
      body = splice(expand(body)),
      caption = caption  // refers to macro-introduced binding
    )

// User code
caption = "My existing caption"
!with_caption(cap: "Figure 1")[Some content here.]
// 'caption' in user scope is NOT shadowed
```

### 5.4 Show/Set Rules (Safe Subset)

For users who need styling without full macro power:

```text
// Set rules: configure defaults
!set heading {
  numbering: "1.1",
  font: "Helvetica",
}

!set code_block {
  theme: "github-dark",
  line_numbers: true,
}

// Show rules: transform rendering
!show heading:
  if it.level == 1:
    page_break() + it
  else:
    it

!show link:
  if it.url.starts_with("http"):
    it + text(" ↗", size: 0.8em)
  else:
    it

// Conditional show rules
!show heading.where(level: 1): set text { color: navy }
```

### 5.5 Macro/Plugin Boundary

**Design decision**: Macros extend the *language*; plugins extend the *editor*.

| Capability                         | Macros                     | Plugins                   |
| ---------------------------------- | -------------------------- | ------------------------- |
| Syntax transformation              | ✓                          | ✗                         |
| Code generation                    | ✓                          | ✗                         |
| Editor commands                    | ✗                          | ✓                         |
| UI elements (panels, decorations)  | ✗                          | ✓                         |
| External I/O (network, filesystem) | Limited (expand-time only) | ✓ (capability-gated)      |
| Runs at                            | Expand-time                | Render-time / Editor-time |

**Plugins cannot define new macros.** This keeps the macro system self-contained and ensures that document semantics don't depend on editor plugins. A document should render identically regardless of which plugins are installed.

**Rationale**: If plugins could define macros, then:

* Document meaning depends on plugin installation state.
* Security model becomes complex (plugin code runs at expand-time with syntax access).
* Caching/incremental computation must account for plugin versions.

---

## 6. Editor Architecture and Extensibility

### 6.1 Facet-Based Extension Model

Following CodeMirror 6, the editor uses **facets** for composable extension:

```rust
/// A facet defines an extension point
pub struct Facet<Input, Output> {
    /// How to combine multiple inputs
    combine: fn(Vec<Input>) -> Output,
    /// Precedence for ordering
    precedence: Precedence,
    /// Whether inputs are deduplicated
    dedupe: bool,
}

/// Core facets
pub static KEYBINDINGS: Facet<Keymap, Keymap> = Facet {
    combine: |maps| maps.into_iter().flatten().collect(),
    precedence: Precedence::Default,
    dedupe: false,
};

pub static DECORATIONS: Facet<DecorationSet, DecorationSet> = Facet {
    combine: |sets| DecorationSet::merge(sets),
    precedence: Precedence::Default,
    dedupe: true,
};

pub static COMMANDS: Facet<CommandMap, CommandMap> = Facet {
    combine: |maps| maps.into_iter().fold(CommandMap::new(), |a, b| a.merge(b)),
    precedence: Precedence::Default,
    dedupe: true,
};
```

### 6.2 WIT Interface Specification

Plugins are WASM components communicating via typed WIT interfaces:

```wit
// monowiki-plugin.wit

package monowiki:plugin@0.1.0;

/// Core types used across interfaces
interface types {
    /// A position in the document
    record position {
        line: u32,
        column: u32,
    }
    
    /// A range in the document
    record range {
        start: position,
        end: position,
    }
    
    /// Text decoration
    record decoration {
        range: range,
        class: string,
        attributes: list<tuple<string, string>>,
    }
    
    /// A diagnostic message
    record diagnostic {
        range: range,
        severity: diagnostic-severity,
        message: string,
        source: option<string>,
    }
    
    enum diagnostic-severity {
        error,
        warning,
        info,
        hint,
    }
    
    /// Selection state
    record selection {
        anchor: position,
        head: position,
        ranges: list<range>,
    }
}

/// Read-only access to document content
interface document-reader {
    use types.{position, range};
    
    /// Get text in a range
    get-text: func(range: range) -> string;
    
    /// Get the full document text
    get-full-text: func() -> string;
    
    /// Get line count
    line-count: func() -> u32;
    
    /// Get line content
    get-line: func(line: u32) -> string;
    
    /// Convert position to byte offset
    position-to-offset: func(pos: position) -> u64;
    
    /// Convert byte offset to position
    offset-to-position: func(offset: u64) -> position;
}

/// Read-write access to document content
interface document-writer {
    use types.{range};
    
    /// Replace text in a range
    replace: func(range: range, text: string);
    
    /// Insert text at position
    insert: func(offset: u64, text: string);
    
    /// Delete text in range
    delete: func(range: range);
}

/// Access to editor UI elements
interface editor-ui {
    use types.{decoration, diagnostic, position};
    
    /// Add decorations to the view
    add-decorations: func(decorations: list<decoration>);
    
    /// Clear decorations from this plugin
    clear-decorations: func();
    
    /// Show diagnostics
    set-diagnostics: func(diagnostics: list<diagnostic>);
    
    /// Show an info message
    show-message: func(message: string);
    
    /// Prompt for user input
    prompt: func(message: string, default: option<string>) -> option<string>;
    
    /// Scroll to position
    scroll-to: func(pos: position);
}

/// Selection and cursor management
interface selection-manager {
    use types.{selection, range, position};
    
    /// Get current selection
    get-selection: func() -> selection;
    
    /// Set selection
    set-selection: func(sel: selection);
    
    /// Get cursor position
    get-cursor: func() -> position;
    
    /// Set cursor position
    set-cursor: func(pos: position);
}

/// Command registration
interface commands {
    /// Register a command that can be invoked by keybinding or command palette
    register-command: func(
        id: string,
        title: string,
        description: option<string>,
    );
}

/// Keybinding registration
interface keybindings {
    /// Bind a key sequence to a command
    bind-key: func(
        key: string,      // e.g., "Ctrl+Shift+P"
        command-id: string,
    );
}

/// Lifecycle hooks
interface lifecycle {
    /// Called when the plugin is activated
    activate: func();
    
    /// Called when the plugin is deactivated
    deactivate: func();
    
    /// Called when document content changes
    on-document-change: func(
        changes: list<tuple<range, string>>,  // (changed range, new text)
    );
    
    /// Called when selection changes
    on-selection-change: func(selection: selection);
}

/// HTTP client (capability-gated)
interface http-client {
    /// Perform an HTTP request
    record http-request {
        method: string,
        url: string,
        headers: list<tuple<string, string>>,
        body: option<list<u8>>,
    }
    
    record http-response {
        status: u16,
        headers: list<tuple<string, string>>,
        body: list<u8>,
    }
    
    /// Send an HTTP request (async)
    fetch: func(request: http-request) -> result<http-response, string>;
}

/// Filesystem access (capability-gated)
interface filesystem {
    /// Read a file's contents
    read-file: func(path: string) -> result<list<u8>, string>;
    
    /// Write to a file
    write-file: func(path: string, contents: list<u8>) -> result<_, string>;
    
    /// List directory contents
    read-dir: func(path: string) -> result<list<string>, string>;
}

/// The world that plugins implement
world plugin {
    // Plugins get read access by default
    import document-reader;
    import editor-ui;
    import selection-manager;
    import commands;
    import keybindings;
    
    // These require explicit capability grants
    import document-writer;  // requires: write
    import http-client;      // requires: network
    import filesystem;       // requires: filesystem
    
    // Plugins export lifecycle hooks
    export lifecycle;
}
```

### 6.3 Capability System

Plugins declare required capabilities in their manifest:

```toml
# plugin.toml
[plugin]
name = "spell-checker"
version = "1.0.0"
description = "Real-time spell checking with suggestions"

[capabilities]
read = true           # Always granted
write = false         # Not needed
network = true        # For dictionary API
filesystem = false    # Not needed

[commands]
spell-check = { title = "Check Spelling", key = "Ctrl+Shift+S" }
add-to-dictionary = { title = "Add to Dictionary" }
```

Users are prompted to grant capabilities on first use:

```text
Plugin "spell-checker" requests:
  ✓ Read document content (always granted)
  ⚠ Network access (for dictionary lookups)
  
[Allow] [Deny] [Allow this session only]
```

### 6.4 Plugin Isolation

Each plugin runs in an isolated WASM instance:

```text
┌─────────────────────────────────────────────────────────────┐
│                      Editor Host (Rust)                     │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐          │
│  │  Plugin A   │  │  Plugin B   │  │  Plugin C   │          │
│  │  (WASM)     │  │  (WASM)     │  │  (WASM)     │          │
│  │             │  │             │  │             │          │
│  │ Memory: 16M │  │ Memory: 8M  │  │ Memory: 32M │          │
│  │ Caps: r,n   │  │ Caps: r,w   │  │ Caps: r     │          │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘          │
│         │                │                │                 │
│         ▼                ▼                ▼                 │
│  ┌─────────────────────────────────────────────────────┐    │
│  │              WIT Interface Boundary                  │   │
│  │  (type-safe, capability-checked, async)              │   │
│  └─────────────────────────────────────────────────────┘    │
│                            │                                │
│                            ▼                                │
│  ┌─────────────────────────────────────────────────────┐    │
│  │                   Core Editor State                  │   │
│  │  (Document, Selections, View, Transactions)          │   │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
```

**Properties**:

* Plugins cannot access host memory directly.
* Crashes are isolated (plugin failure ≠ editor crash).
* Resource limits (memory, CPU time) per plugin.
* All I/O is mediated through capability-checked interfaces.

### 6.5 Syndicated Actor Runtime

The WIT-based WASM plugin host specifies *what* plugins can do. The **syndicated actor runtime** specifies *how* they interact and are isolated at runtime.

#### 6.5.1 Actors and Dataspaces

We instantiate the actor model as follows:

**Actors:**

* Each **plugin instance** is an actor.
* Each **code execution kernel** (JS/WASM) is an actor.
* Each **connected client** (browser tab, desktop app window) is an actor.
* System services (presence, search indexer, export pipeline) are actors.

**Dataspaces:**

* `doc-content/<doc-id>` — views over CRDT state:
  * `BlockInfo(node_id, parent_id, kind, order, attrs)`
  * `BlockText(block_id, text: String)` — full text of a block
  * `BlockChanged(block_id, range, new_text)` — incremental change notifications
  * `MarkInfo(block_id, mark_id, type, start, end, attrs)`

* `doc-view/<doc-id>` — ephemeral editor/view-related assertions:
  * `Cursor(user_id, pos)`
  * `Selection(user_id, ranges)`
  * `Decoration(plugin_id, range, class, attrs)`
  * `Diagnostic(plugin_id, range, severity, message)`
  * `EvalRequest(cell_id, kernel_id, code_hash)`
  * `EvalResult(cell_id, code_hash, status, payload_ref)`

* `system` — global services:
  * `PluginRegistered(id, manifest, capabilities)`
  * `CapabilityGrant(subject, dataspace_ref, policy)`
  * `DocumentOpened(user_id, doc_id)`
  * `WorkspaceConfig(workspace_id, settings)`

**Assertion granularity:** Assertions in `doc-content` expose *views* over the CRDT state, not raw CRDT operations. The granularity is configurable per subscription:
- A spellchecker might subscribe to `BlockText(block_id, text)` updates.
- A fine-grained diff tool might subscribe to `BlockChanged(block_id, range, new_text)`.
- A structural outline might subscribe only to `BlockInfo` for heading blocks.

The host translates CRDT changes into assertion updates at the appropriate granularity.

#### 6.5.2 Mapping WIT Interfaces onto Actors

The existing WIT interfaces map onto the actor model:

* `document-reader` is implemented as:
  * A view over assertions in `doc-content/<doc-id>`.
  * Utilities for converting between `(line, column)` and internal positions.

* `document-writer` is implemented as:
  * Host-side translation from requested edits into CRDT operations.
  * CRDT updates then propagate to `doc-content` assertions.

* `editor-ui` and `selection-manager` manipulate:
  * Assertions in `doc-view/<doc-id>` (`Decoration`, `Diagnostic`, `Selection`, `Cursor`).

**Which interface do plugins see?**

Plugins use the high-level WIT interfaces (`document-reader`, `editor-ui`, etc.). The dataspace abstraction is host-internal. This keeps the plugin API simple and safe.

In the future, advanced plugins may opt into a lower-level dataspace interface (with appropriate capability grants), but this is not part of the initial design.

#### 6.5.3 Capability-Based Sandboxing

Sandboxing is enforced in two layers:

1. **WASM sandbox** (memory, CPU, host API surface).

2. **Object-capability discipline at the actor level**:
   * Each plugin or kernel actor obtains capabilities to specific dataspaces.
   * Capabilities are attenuated:
     * Limit the *types* of assertions an actor can publish (e.g., `Diagnostic`, not `BlockText`).
     * Limit which assertions they can observe (e.g., only blocks in their document).

**Examples:**

* A **spellchecker plugin**:
  * Observes `BlockText` assertions in `doc-content/<doc-id>`.
  * Publishes `Diagnostic` assertions in `doc-view/<doc-id>`.
  * Has no capability for document writes or `EvalRequest`.

* A **code execution kernel**:
  * Subscribes to `EvalRequest` for its `kernel_id` in `doc-view/<doc-id>`.
  * Publishes `EvalResult` assertions.
  * Receives a limited capability for temporary storage and outbound HTTP (if granted).

Capability assignments are stored as assertions in the `system` dataspace, enabling audit and inspection.

#### 6.5.4 Actor Lifecycle and Failure Handling

**Lifecycle:**

* When a user opens a document:
  * A client actor joins `doc-content/<doc-id>` and `doc-view/<doc-id>`.
  * The host publishes `Presence(user_id, doc_id, session_id)` assertions.

* When a plugin is enabled:
  * A plugin actor is started.
  * It subscribes to relevant patterns (e.g., `BlockText` or `Diagnostic`).

* When the document closes:
  * The host retracts presence assertions.
  * Plugin actors either terminate or leave the dataspaces.

**Failure handling:**

* **Plugin crash:** The actor terminates; its assertions (e.g., `Diagnostic`, `Decoration`) are automatically retracted. Other actors can observe this and respond (e.g., the UI removes stale decorations). The host can restart the plugin actor if configured to do so.

* **Kernel timeout:** The host monitors `EvalRequest` assertions. If a kernel actor doesn't publish a corresponding `EvalResult` within a timeout, the host can:
  * Retract the `EvalRequest`.
  * Publish an `EvalResult` with status `timeout`.
  * Optionally terminate and restart the kernel actor.

* **Client disconnect:** The client actor terminates; its `Cursor` and `Selection` assertions are retracted. Other clients see presence updates.

This model makes failure handling explicit and compositional: assertion retraction is the universal signal that something is gone.

#### 6.5.5 Example: Spellchecker Plugin Flow

A concrete example showing how the pieces fit together:

```text
1. User types "teh " in block B
   │
   ▼
2. Editor captures keystroke, creates Transaction
   │
   ▼
3. Transaction applied to CRDT layer (Loro)
   │
   ▼
4. CRDT actor asserts into doc-content/<doc-id>:
   BlockChanged(B, range(3,3), "teh ")
   BlockText(B, "The quick teh brown fox")
   │
   ▼
5. Spellchecker actor (subscribed to BlockText) receives update
   │
   ▼
6. Spellchecker runs check, finds "teh" is misspelled
   │
   ▼
7. Spellchecker asserts into doc-view/<doc-id>:
   Diagnostic(spellcheck-plugin, range(10,13), warning, "Did you mean 'the'?")
   │
   ▼
8. Editor actor (subscribed to Diagnostic) receives assertion
   │
   ▼
9. Editor renders squiggly underline at range(10,13)

**Notebook reflection and kernels:** A document can request derived data about itself at expand-time (e.g., `!staged[outline = doc.outline(); for h in outline: paragraph(h.title)]`), and shell out to kernels/agents at render-time via EvalRequest/Result assertions. Render-time is JS/WASM by default; other languages must arrive via sandboxed kernels.
```

If the spellchecker plugin crashes at step 6:
- Its `Diagnostic` assertions are retracted.
- The editor sees the retraction and removes any squiggles from that plugin.
- The host can restart the plugin; it will re-process current `BlockText` assertions.

---

## 7. CRDT Design and Collaboration

### 7.1 Data Model

The operational representation uses three CRDT layers:

```text
┌───────────────────────────────────────────────────────────────┐
│                    Document CRDT Structure                    │
├───────────────────────────────────────────────────────────────┤
│                                                               │
│  Layer 1: MovableTree (Document Structure)                    │
│  ┌─────────────────────────────────────────────────────────┐  │
│  │  Document                                                │ │
│  │  ├── Section "intro" (order: 0.0)                        │ │
│  │  │   ├── Block "h1" (order: 0.0)                         │ │
│  │  │   └── Block "p1" (order: 1.0)                         │ │
│  │  ├── Section "methods" (order: 1.0)                      │ │
│  │  │   ├── Block "h2" (order: 0.0)                         │ │
│  │  │   ├── Block "code1" (order: 1.0)                      │ │
│  │  │   └── Block "p2" (order: 2.0)                         │ │
│  │  └── ...                                                 │ │
│  └─────────────────────────────────────────────────────────┘  │
│                                                               │
│  Layer 2: Fugue (Text Sequences)                              │
│  ┌─────────────────────────────────────────────────────────┐  │
│  │  Block "p1" text:                                        │ │
│  │  [id:a1]'H' ← [id:a2]'e' ← [id:a3]'l' ← [id:a4]'l' ...   │ │
│  │                                                          │ │
│  │  Block "code1" text:                                     │ │
│  │  [id:b1]'d' ← [id:b2]'e' ← [id:b3]'f' ← [id:b4]' ' ...   │ │
│  └─────────────────────────────────────────────────────────┘  │
│                                                               │
│  Layer 3: Peritext (Formatting Marks)                         │
│  ┌─────────────────────────────────────────────────────────┐  │
│  │  Mark { type: "emphasis", start: a1, end: a4,            │ │
│  │         startAnchor: before, endAnchor: before }         │ │
│  │                                                          │ │
│  │  Mark { type: "link", start: a5, end: a9,                │ │
│  │         startAnchor: before, endAnchor: after,           │ │
│  │         attrs: { href: "https://..." } }                 │ │
│  └─────────────────────────────────────────────────────────┘  │
│                                                               │
└───────────────────────────────────────────────────────────────┘
```

### 7.2 Operations

```rust
// Tree operations (via MovableTree)
enum TreeOp {
    /// Insert a new node
    Insert {
        id: OpId,
        parent: NodeId,
        position: FractionalIndex,
        node_type: NodeType,
    },
    /// Move a node to a new parent/position
    Move {
        id: OpId,
        node: NodeId,
        new_parent: NodeId,
        new_position: FractionalIndex,
    },
    /// Delete a node (tombstone)
    Delete {
        id: OpId,
        node: NodeId,
    },
}

// Text operations (via Fugue)
enum TextOp {
    Insert {
        id: OpId,
        block: NodeId,
        position: FuguePosition,
        content: String,
    },
    Delete {
        id: OpId,
        block: NodeId,
        start: CharId,
        end: CharId,
    },
}

// Formatting operations (via Peritext)
enum MarkOp {
    AddMark {
        id: OpId,
        block: NodeId,
        mark_type: String,
        start: CharId,
        end: CharId,
        start_anchor: Anchor,
        end_anchor: Anchor,
        attrs: HashMap<String, Value>,
    },
    RemoveMark {
        id: OpId,
        block: NodeId,
        mark_type: String,
        start: CharId,
        end: CharId,
    },
}
```

### 7.3 Sync Protocol

```text
┌─────────────┐                              ┌─────────────┐
│   Client A  │                              │   Client B  │
└──────┬──────┘                              └──────┬──────┘
       │                                            │
       │  1. Local edit: insert "Hello"             │
       │  ─────────────────────────────►            │
       │  (optimistic local apply)                  │
       │                                            │
       │  2. Sync to server                         │
       │  ══════════════════════════════►           │
       │  {ops: [Insert{...}], vector: {...}}       │
       │                                            │
       │                    ┌─────────┐             │
       │                    │ Server  │             │
       │                    │ (Loro)  │             │
       │                    └────┬────┘             │
       │                         │                  │
       │                         │ 3. Validate &    │
       │                         │    persist       │
       │                         │                  │
       │                         │ 4. Broadcast     │
       │  ◄══════════════════════╪══════════════════╡
       │  {ops: [...], vector: {...}}               │
       │                                            │
       │  5. Merge (CRDT)        │  5. Merge (CRDT) │
       │                         │                  │
       ▼                         ▼                  ▼
```

### 7.4 Computed Values: Don't Sync

For reactive documents, computed values are **not synced**:

```text
Source (synced via CRDT):
  - Code cell: "x = [1, 2, 3]"
  - Code cell: "sum(x)"

NOT synced (recomputed locally after merge):
  - Output of cell 1: [1, 2, 3]
  - Output of cell 2: 6
  - Rendered layout
  - Generated figures
```

**Rationale**: If Client A edits cell 1 to `x = [1, 2, 3, 4]` while Client B edits cell 2 to `sum(x) * 2`, syncing outputs would produce incoherent results. Instead:

1. CRDT syncs source code.
2. After merge, each client re-evaluates.
3. All clients converge to the same outputs (given the same inputs).

### 7.5 Semantic Conflict Detection

After CRDT merge, a validation layer checks semantic invariants:

```rust
struct SemanticValidator {
    validators: Vec<Box<dyn Validator>>,
}

trait Validator {
    fn validate(&self, content: &Content) -> Vec<SemanticConflict>;
}

struct SemanticConflict {
    kind: ConflictKind,
    locations: Vec<SourceLocation>,
    message: String,
    suggestions: Vec<Suggestion>,
}

enum ConflictKind {
    BrokenReference,      // @fig-foo points to deleted figure
    DuplicateId,          // Two elements have same #id
    TypeMismatch,         // Code cell type error after merge
    CyclicDependency,     // Cells form dependency cycle
    SchemaViolation,      // Directive args don't match schema
}
```

These are surfaced to users as distinct from syntax errors—they arise from concurrent edits, not individual mistakes.

### 7.6 Persistent State vs Conversational State

CRDTs and dataspaces both involve replicated state, but serve different purposes:

**CRDT layer (Loro):**
* Represents **persistent document state**: text, structure, formatting marks.
* Provides offline editing, eventual consistency, and conflict-free merges.
* Is purely *application data*; it does not model conversations or runtime behavior.

**Dataspaces in the actor runtime:**
* Represent **conversational, ephemeral, and supervisory state**:
  * Live diagnostics, status messages, presence, service discovery, supervision.
* Assertions are **tied to actor lifetimes** and withdrawn automatically on failure or shutdown.
* Express many coordination patterns (pub/sub, service discovery, monitors) as assertion protocols rather than ad hoc channels.

**Design rules:**

1. **CRDTs own the document.** Dataspaces never attempt to represent canonical document content as mutable application state.

2. **Dataspaces own conversations and coordination:**
   * Presence: "user A has document D open"
   * Service discovery: "this plugin provides a linter for language X"
   * Supervision: "this code cell is executing"

3. **Bridges, not duplication:**
   * CRDT actors assert summary facts into dataspaces (e.g., `DocVersion(doc_id, hash)` or `SyncStatus(doc_id, remote_id, lag)`).
   * Conversational state (e.g., "user A is requesting a fresh render") is not written back into CRDTs.

This separation prevents subtle feedback loops and keeps "forever" state (documents) distinct from "right now" state (conversations).

### 7.7 Distributed Dataspaces

In multi-client deployments, dataspaces span processes and machines:

**`doc-content` assertions:**
* Derived from CRDT state, so they converge automatically via CRDT sync.
* Each client maintains a local view; CRDT ops keep them consistent.

**`doc-view` assertions:**
* Presence and cursors: replicated to all clients (via sync protocol).
* Diagnostics: typically *not* replicated (each client runs its own plugins).
* Eval state: optionally replicated (depends on whether computation is shared or per-client).

The sync protocol carries both:
* CRDT operations (for document convergence).
* Assertion updates (for presence and selected `doc-view` state).

**Consistency model:** Dataspaces provide eventual consistency. An assertion published by Client A will eventually be visible to Client B, but there's no strong ordering guarantee across clients. For document content, the CRDT provides the ordering semantics; dataspaces only reflect that state.

### 7.8 Server Authority (Optional)

For documents with complex invariants, optional server validation:

```rust
// Server-side validation hook
async fn validate_operations(
    doc: &Document,
    ops: Vec<Operation>,
) -> Result<Vec<Operation>, ValidationError> {
    // Apply ops to copy of state
    let mut draft = doc.clone();
    for op in &ops {
        draft.apply(op)?;
    }
    
    // Check invariants
    let violations = semantic_validator.validate(&draft)?;
    
    if violations.is_empty() {
        Ok(ops)  // Accept
    } else {
        Err(ValidationError::SemanticConflicts(violations))
    }
}
```

**Trade-off**: Server authority provides stronger guarantees but requires connectivity. The system should work in pure P2P mode with eventual consistency; server validation is an optional layer for teams that want stricter invariants.

---

## 8. System Architecture

### Language Detail References

- **Monowiki Reflective Language (MRL):** see `vault/mrl.md` for the staged language spec (single `!` escape, Python-flavored macros, JS/WASM render-time).

### 8.1 High-Level Data and Actor Flow

```text
┌─────────────────────────────────────────────────────────────────────┐
│                         MONOWIKI ARCHITECTURE                       │
└─────────────────────────────────────────────────────────────────────┘

  ┌──────────────────┐
  │   User Input     │
  │ (keystrokes,     │
  │  pointer, etc.)  │
  └────────┬─────────┘
           │ events
           ▼
┌─────────────────────────────────────────────────────────────────────┐
│                     SYNDICATED ACTOR RUNTIME                        │
│ (turn-based scheduler, dataspaces, capabilities, supervision)       │
├─────────────────────────────────────────────────────────────────────┤
│  Actors:                                                            │
│   • Editor actor                                                    │
│   • Document engine actor                                           │
│   • CRDT / sync actor(s)                                            │
│   • Plugin actors (WASM)                                            │
│   • Code execution actors (per kernel)                              │
│   • Presence / awareness actors                                     │
│   • Server / gateway actors (optional)                              │
│                                                                     │
│  Dataspaces:                                                        │
│   • system                                                          │
│   • doc-content/<doc-id>                                            │
│   • doc-view/<doc-id>                                               │
└───────────────┬─────────────────────────────────────────────────────┘
                │ assertions / subscriptions
                ▼
┌─────────────────────────────────────────────────────────────────────┐
│                    OPERATIONAL / DATA LAYERS                        │
│                                                                     │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │ CRDT LAYER (Loro)                                              │ │
│  │  • MovableTree (structure) + Fugue (text) + Peritext (marks)   │ │
│  │  • Sync protocol for multi-client replication                  │ │
│  └───────────────────────────────────────────────────────────────┘  │
│                         │                                           │
│                         │ invalidates                               │
│                         ▼                                           │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │ INCREMENTAL COMPUTATION LAYER (Salsa-inspired queries)        │  │
│  │  • source_text → parse_shrubbery → expand_to_content          │  │
│  │  • resolve_references → layout_section → render_view          │  │
│  │  • Early cutoff, durability tiers, content-addressable cache  │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                         │                                           │
│                         │ produces                                  │
│                         ▼                                           │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │ SEMANTIC LAYER                                                 │ │
│  │  • Shrubbery parser                                            │ │
│  │  • Macro expander + hygiene + typed staging                    │ │
│  │  • Show/set rule engine                                        │ │
│  │  • Semantic validators (cross-refs, types, schema)             │ │
│  └───────────────────────────────────────────────────────────────┘  │
│                                                                     │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │ CODE EXECUTION SANDBOXES (WASM)                                │ │
│  │  • JS/WASM kernels                                            │ │
│  │  • Each wrapped by a kernel actor                              │ │
│  │  • Results asserted into doc-view dataspaces                   │ │
│  └───────────────────────────────────────────────────────────────┘  │
└────────────────────────┬────────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────────────┐
│                          RENDERING LAYER                            │
│   • Layout engine, syntax highlighting                              │
│   • Interactive widgets                                             │
│   • Export (HTML, PDF, etc.)                                        │
│   • DOM / Canvas (browser)                                          │
└─────────────────────────────────────────────────────────────────────┘
```

### 8.2 Phase Boundaries

```text
┌─────────────────────────────────────────────────────────────────────────┐
│                         STAGING BOUNDARIES                              │
└─────────────────────────────────────────────────────────────────────────┘

   Source Text
       │
       │  ════════════════════════════════════════════
       │  READ-TIME
       │  ════════════════════════════════════════════
       │
       ▼
   Shrubbery (token tree with grouping)
       │
       │  ════════════════════════════════════════════
       │  EXPAND-TIME
       │  
       │  • Macro expansion
       │  • !staged blocks executed
       │  • Type checking (structural)
       │  • Cross-reference resolution
       │  • Show/set rule application
       │  ════════════════════════════════════════════
       │
       ▼
   Content (typed document tree)
       │
       │  ════════════════════════════════════════════
       │  RENDER-TIME
       │  
       │  • Layout computation
       │  • Live code execution (#| live: true)
       │  • Interactive widget binding
       │  • User event handling
       │  ════════════════════════════════════════════
       │
       ▼
   Rendered Output (DOM, PDF, etc.)
```

### 8.3 Query Dependency Graph

```text
┌─────────────────────────────────────────────────────────────────────────┐
│                    QUERY DEPENDENCY GRAPH                               │
└─────────────────────────────────────────────────────────────────────────┘

Volatile tier (invalidated by edits):

  crdt_state(doc)
       │
       ├──► source_text(section) ──► parse_shrubbery(section)
       │           │                         │
       │           │                         ▼
       │           │               expand_to_content(section)
       │           │                         │
       │           │                         │    ┌─────────────────────┐
       │           │                         │    │ Early cutoff:       │
       │           │                         │    │ If AST unchanged,   │
       │           │                         │    │ skip downstream     │
       │           │                         │    └─────────────────────┘
       │           │                         ▼
       │           │               resolve_references(doc)
       │           │                         │
       │           ▼                         ▼
       │    layout_section(section, viewport)
       │           │
       │           ▼
       └──► render_view(viewport)


Session tier (invalidated by user actions):

  viewport_state ──► visible_sections ──► demanded queries


Durable tier (invalidated by explicit reload):

  theme_config ──► style_rules ──► layout parameters
       │
  macro_definitions ──► expander configuration
       │
  plugin_state ──► editor extensions


Static tier (never invalidated):

  builtin_functions
  core_types
  standard_macros
```

### 8.4 Relating Queries, Actors, and Dataspaces

```text
┌─────────────────────────────────────────────────────────────────────────┐
│              HOW QUERIES AND ACTORS INTERACT                            │
└─────────────────────────────────────────────────────────────────────────┘

                    ┌─────────────────────────────────────┐
                    │           ACTORS                    │
                    │  (plugins, kernels, clients)        │
                    └──────────────┬──────────────────────┘
                                   │
                    publish/subscribe via assertions
                                   │
                                   ▼
                    ┌─────────────────────────────────────┐
                    │          DATASPACES                 │
                    │  doc-content, doc-view, system      │
                    └──────────────┬──────────────────────┘
                                   │
              ┌────────────────────┴────────────────────┐
              │                                         │
              ▼                                         ▼
   ┌─────────────────────┐                 ┌─────────────────────┐
   │   CRDT LAYER        │                 │   QUERY LAYER       │
   │   (Loro)            │ ───────────────►│   (Salsa-inspired)  │
   │                     │   invalidates   │                     │
   │   Source of truth   │                 │   Derived values    │
   │   for document      │                 │   (AST, layout)     │
   └─────────────────────┘                 └──────────┬──────────┘
                                                      │
                                              projects outputs
                                                      │
                                                      ▼
                                           ┌─────────────────────┐
                                           │   DATASPACES        │
                                           │   (as assertions)   │
                                           │                     │
                                           │   e.g., Outline,    │
                                           │   SemanticDiagnostic│
                                           └─────────────────────┘

Key flows:

1. CRDT change → invalidates queries → query recomputes → 
   output projected into dataspace → actors observe new assertions

2. Actor publishes assertion (e.g., EvalRequest) → 
   other actor observes → responds with assertion (EvalResult)

3. Queries NEVER called by actors directly; actors see only assertions
```

---

## 9. Implementation Roadmap

### Phase 1: Core Document Model (Months 1–3)

**Goal**: End-to-end rendering of static documents with Djot-style syntax.

**Deliverables**:

* [ ] Djot-style parser producing shrubbery
* [ ] `Content` type hierarchy implementation
* [ ] Basic `!` macro invocation for block/inline content (role-like)
* [ ] In-memory document representation (pre-CRDT)
* [ ] Simple HTML renderer
* [ ] Test suite with golden files

**Non-goals**: Macros, collaboration, code execution.

### Phase 2: Incremental Computation (Months 4–5)

**Goal**: Fast re-rendering on edits with early cutoff.

**Deliverables**:

* [ ] Salsa-inspired query system
* [ ] Query dependency tracking
* [ ] Early cutoff implementation
* [ ] Durability tiers
* [ ] Content-addressable section cache
* [ ] Benchmarks: edit-to-render latency < 16ms for typical edits

**Non-goals**: CRDT integration (still single-user).

### Phase 3: Macro System and Staging (Months 6–8)

**Goal**: User-definable syntax extensions with typed staging.

**Deliverables**:

* [ ] Shrubbery → enforestation pipeline
* [ ] Syntax object representation with hygiene
* [ ] `Code<K>` staged types
* [ ] Type checker for staged document fragments
* [ ] Show/set rule engine
* [ ] `!staged` block execution (expand-time)
* [ ] `!show`/`!set` expand-time styling (no shrubbery access)
* [ ] Macro authoring API and documentation
* [ ] Standard library of common macros (callouts, tabs, figures)

**Non-goals**: Full user-facing macro DSL (just API).

### Phase 4: Editor, Actor Runtime, and Extensibility (Months 9–11)

**Goal**: Interactive editing with plugin support on top of a minimal actor runtime.

**Deliverables**:

* [ ] Facet-based extension model
* [ ] Transaction system for atomic edits
* [ ] WASM plugin host (Wasmtime)
* [ ] WIT interface implementation
* [ ] Capability system and permission prompts
* [ ] Minimal syndicated actor runtime (single-process):
  * In-process implementation of actors + dataspaces
  * `system`, `doc-content/<doc-id>`, and `doc-view/<doc-id>` dataspaces
  * Mapping of WIT interfaces onto dataspace assertions
* [ ] Core plugins: spell check, word count, outline
* [ ] Plugin authoring documentation

**Non-goals**: Public plugin marketplace.

### Phase 5: Collaboration (Months 12–14)

**Goal**: Real-time multiplayer editing.

**Deliverables**:

* [ ] Loro integration for CRDT layer
* [ ] MovableTree for document structure
* [ ] Fugue for text sequences
* [ ] Peritext for formatting marks
* [ ] Sync protocol implementation
* [ ] Presence/awareness (cursors, selections)
* [ ] Semantic conflict detection
* [ ] Optional server validation mode
* [ ] Offline support with sync-on-reconnect
* [ ] Distributed dataspaces:
  * Replicate `doc-content` assertions via CRDT sync
  * Replicate presence/cursor assertions across clients
  * Stable identity scheme for dataspaces

### Phase 6: Code Execution (Months 15–16)

**Goal**: Reactive computation within documents.

**Deliverables**:

* [ ] WASM-sandboxed code execution
* [ ] JS/WASM support (default render-time runtime)
* [ ] JavaScript support
* [ ] `#| live: true` reactive cells
* [ ] Dependency tracking between cells
* [ ] Output caching (don't sync computed values)
* [ ] Timeout and resource limits
* [ ] Code kernels as actors:
  * Each kernel subscribes to `EvalRequest` assertions
  * Kernel outputs are `EvalResult` assertions
  * Capability-gated resource access

### Phase 7: Polish and Ecosystem (Months 17–18)

**Goal**: Production readiness.

**Deliverables**:

* [ ] PDF export
* [ ] Import from Markdown/CommonMark
* [ ] Theme system
* [ ] Accessibility audit
* [ ] Performance optimization
* [ ] Documentation site (dogfooding!)
* [ ] Public beta

---

## 10. Trade-offs and Open Questions

### 10.1 Accepted Trade-offs

| Trade-off                    | Choice                                           | Rationale                                                                                                                  |
| ---------------------------- | ------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------- |
| **Typing discipline**        | Static for staged fragments, dynamic for runtime | Catches structural errors early; allows flexible embedded code                                                             |
| **CommonMark compatibility** | Djot-style (incompatible)                        | Unambiguous parsing, better editor behavior                                                                                |
| **Plugin power**             | Editor-only (no macro registration)              | Document semantics independent of plugins                                                                                  |
| **Process model**            | In-process WASM                                  | Performance, simplicity; isolation via WASM                                                                                |
| **Collaboration model**      | CRDT + optional server                           | Works offline; server adds invariant checking                                                                              |
| **Concurrency model**        | Syndicated actors over ad hoc event buses        | Unifies plugins, kernels, and clients under a single capability-secure model                                               |
| **Plugin API level**         | High-level WIT (not raw dataspaces)              | Simpler plugin authoring; raw dataspaces as future opt-in                                                                  |
| **Actor boundary**           | Extensions only (pragmatic)                      | Core stays as direct host code; actor model for boundaries                                                                 |

### 10.2 Open Questions

**Q1: How fine-grained should CRDT tracking be?**

Options:

* Character-level (current design): maximum precision, higher overhead.
* Word-level: lower overhead, coarser merges.
* Block-level: simplest, but loses concurrent intra-block edits.

**Recommendation**: Start with character-level (via Loro), optimize later if needed. The incremental computation layer should absorb most costs.

**Q2: Should there be a "document type" system?**

Some documents might declare a schema:

```markdown
!doctype(name: "article")[
  requires: [title, abstract, sections]
  forbids: [raw-html]
]
```

This would enable:

* Validation beyond structural typing.
* Editor affordances (required field warnings).
* Export guarantees (e.g., "this document can become a valid PDF").

**Recommendation**: Defer to Phase 7. The semantic validator infrastructure supports this, but schema design needs user research.

**Q3: How should cross-document references work?**

Options:

* File-based: `@other-doc.md#sec-intro`
* Wiki-style: `[[Other Document#Introduction]]`
* Database-style: global ID namespace

**Recommendation**: Start with file-based (closest to existing tools), add wiki-style as sugar. Global IDs require significant infrastructure.

**Q4: What's the plugin update/versioning story?**

Plugins will evolve. Need to handle:

* Breaking WIT changes.
* Plugin state migration.
* User expectations of stability.

**Recommendation**: Semver for WIT interfaces; plugins declare compatible interface versions. Breaking changes bump major version; old interfaces supported for one major version.

**Q5: Should the raw dataspace interface be exposed to plugins?**

Options:

* **High-level only (current choice)**: Plugins use `document-reader`, `editor-ui`, etc. Simpler, safer.
* **Opt-in raw access**: Advanced plugins can request dataspace capabilities. More flexible, more complex.
* **Low-level only**: Everything is dataspace operations. Elegant but abstract.

**Recommendation**: Start with high-level only. Add opt-in raw access in a later phase if plugin authors request it.

---

## References

### Document Languages and Calculi

* Krishnamurthi & Krishnamurthi. "A Core Calculus for Documents." Brown PLT, 2023.
* Typst documentation.
* Pollen documentation (Racket).

### Incremental Computation

* Hammer et al. "Adapton: Composable, Demand-Driven Incremental Computation." PLDI 2014.
* Salsa documentation and blog posts.

### Macro Systems

* Flatt. "Binding as Sets of Scopes." POPL 2016.
* Rhombus documentation (Racket).

### Editor Architecture

* Haverbeke. "Facets as Composable Extension Points." 2020.
* CodeMirror 6 documentation.
* Levien. "Xi-editor retrospective." 2020.

### CRDTs and Collaboration

* Litt et al. "Peritext: A CRDT for Rich-Text Collaboration." CSCW 2022.
* Kleppmann et al. "A Highly-Available Move Operation for Replicated Trees." IEEE 2021.
* Loro documentation.

### Syntax Design

* MacFarlane. "Beyond Markdown." 2022.
* Djot documentation.
* MyST documentation.

### Syndicated Actor Model

* Garnock-Jones, T. "Syndicated Actors" and related writings.
* Synit documentation.
