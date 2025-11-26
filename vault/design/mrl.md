# Monowiki Reflective Language (MRL) Specification

**Version**: 0.1.0-draft  
**Status**: Design Document

---

## Table of Contents

1. [Overview and Design Goals](#1-overview-and-design-goals)
2. [Surface Grammar](#2-surface-grammar)
3. [Type and Kind System](#3-type-and-kind-system)
4. [AST and Syntax Objects](#4-ast-and-syntax-objects)
5. [Expand-Time Interpreter](#5-expand-time-interpreter)
6. [WASM Runtime ABI](#6-wasm-runtime-abi)
7. [Codegen Strategy](#7-codegen-strategy)
8. [Standard Library](#8-standard-library)
9. [Examples and Golden Tests](#9-examples-and-golden-tests)
10. [Security Model](#10-security-model)
11. [Open Questions and Risks](#11-open-questions-and-risks)
12. [Implementation Roadmap](#12-implementation-roadmap)

---

## 1. Overview and Design Goals

### 1.1 What is MRL?

MRL (Monowiki Reflective Language) is a small, typed, expression-oriented language embedded in monowiki documents. It provides:

- **Unified escape**: A single `!` character transitions from prose to code
- **Staged execution**: Expand-time (compile-time) and render-time (runtime) phases
- **Hygienic macros**: Safe code generation with lexical scoping guarantees
- **Document reflection**: Introspection of document structure, references, and metadata
- **Actor integration**: Communication with kernels, plugins, and dataspaces

### 1.2 Design Principles

| Principle | Implication |
|-----------|-------------|
| **Prose-first** | Markdown-like content is the default; code is the exception |
| **Single escape** | Only `!` transitions to code; no directive soup |
| **Staged safety** | Generated code is well-typed by construction |
| **Capability-secure** | I/O requires explicit capability grants |
| **Incremental** | All constructs support fine-grained recomputation |

### 1.3 Execution Model

```
┌─────────────────────────────────────────────────────────────────┐
│                     DOCUMENT LIFECYCLE                          │
└─────────────────────────────────────────────────────────────────┘

  Source Text (Markdown + MRL)
       │
       ▼
  ┌─────────────────────────────────────────────────────────────┐
  │  READ-TIME (Parser)                                         │
  │  - Tokenize prose and code                                  │
  │  - Build shrubbery (token tree with scopes)                 │
  │  - No evaluation, no macro expansion                        │
  └─────────────────────────────────────────────────────────────┘
       │
       ▼  Shrubbery
  ┌─────────────────────────────────────────────────────────────┐
  │  EXPAND-TIME (Host Interpreter)                             │
  │  - Macro expansion with hygiene                             │
  │  - !staged[...] blocks executed                             │
  │  - Type checking of staged fragments                        │
  │  - Show/set rules applied                                   │
  │  - Document reflection (outline, refs)                      │
  │  - Output: Content tree + WASM modules for live cells       │
  └─────────────────────────────────────────────────────────────┘
       │
       ▼  Content + WASM
  ┌─────────────────────────────────────────────────────────────┐
  │  RENDER-TIME (Browser/WASM Runtime)                         │
  │  - WASM modules instantiated                                │
  │  - Reactive signals evaluated                               │
  │  - UI widgets rendered                                      │
  │  - User interaction handled                                 │
  │  - Kernel communication via dataspaces                      │
  └─────────────────────────────────────────────────────────────┘
       │
       ▼
  Rendered Output (DOM)
```

---

## 2. Surface Grammar

### 2.1 Design: The `!` Escape

The exclamation mark `!` is the **sole** escape from prose to code. This choice:

- Avoids the "directive explosion" of MyST/RST
- Is visually distinct and easy to type
- Naturally suggests "action" or "evaluation"
- Doesn't conflict with Markdown (where `!` only matters before `[`)

**Escape rules:**
- `!` followed by identifier or `(` or `[` → code mode
- `!!` → literal `!` character
- `!` at end of word or followed by space/punctuation → literal `!`

### 2.2 EBNF Grammar (normalized for `!`, remove directives)

```ebnf
(* ===== Document Level ===== *)

document     = { prose_or_code } ;

prose_or_code = prose_span
              | inline_code
              | block_code ;

prose_span   = { char - "!" | "!!" | "!" lookahead_not_code } ;

(* ===== Inline Code ===== *)

inline_code  = "!", inline_expr ;

inline_expr  = identifier                           (* variable reference *)
             | identifier, "(", [ args ], ")"       (* function call *)
             | identifier, "[", content_block, "]"  (* call with content arg *)
             | identifier, "(", [ args ], ")", "[", content_block, "]"
             | "(", expr, ")"                       (* parenthesized expr *)
             | "[", content_block, "]"              (* content literal *)
             ;

(* ===== Block Code ===== *)

block_code   = block_def
             | block_staged
             | block_show
             | block_set
             | block_live ;

block_def    = "!def", identifier, [ "(", params, ")" ], [ ":", type ],
               newline, indent, block_body, dedent
             | "!def", identifier, [ "(", params, ")" ], [ ":", type ],
               "=", expr ;

block_body   = { stmt, newline }, expr ;

stmt         = identifier, "=", expr
             | expr ;

block_staged = "!staged", newline, indent, block_body, dedent
             | "!staged", "[", block_body, "]" ;

block_show   = "!show", selector, ":", transform_expr, newline
             | "!show", selector, newline, indent, transform_body, dedent ;

block_set    = "!set", selector, "{", properties, "}", newline ;

block_live   = "!live", [ "(", live_opts, ")" ], newline,
               indent, block_body, dedent ;

(* ===== Expressions ===== *)

expr         = literal
             | identifier
             | expr "." identifier                 (* field access *)
             | expr "." identifier "(" [ args ] ")"  (* method call *)
             | expr "(" [ args ] ")"              (* function call *)
             | expr "[" expr "]"                  (* subscript *)
             | expr binop expr                    (* binary op *)
             | unop expr                          (* unary op *)
             | "if" expr ":" newline indent expr dedent "else" ":" newline indent expr dedent
             | "for" pattern "in" expr ":" newline indent expr dedent
             | "def" "(" params ")" ":" newline indent expr dedent  (* lambda-like block *)
             | quote_expr
             | splice_expr
             | content_block
             | "(" expr ")" ;

quote_expr   = "quote", ":", newline, indent, expr, dedent
             | "quote", "[", content_block, "]"
             | "'[", content_block, "]" ;  (* type rules use '[...] as shorthand *)

splice_expr  = "splice", "(", expr, ")"              (* splice expr *)
             | "$", identifier                       (* short splice *)

content_block = "[", { prose_or_code }, "]" ;

(* ===== Selectors (for show/set) ===== *)

selector     = type_selector, [ where_clause ] ;

type_selector = "heading" | "paragraph" | "code_block" | "link"
              | "image" | "list" | "blockquote" | "table"
              | "emphasis" | "strong" | "math" ;

where_clause = ".where", "(", predicate_expr, ")" ;

predicate_expr = identifier, "==", literal
               | identifier, ".", "starts_with", "(", string, ")"
               | identifier, ".", "matches", "(", regex, ")"
               | predicate_expr, "&&", predicate_expr
               | predicate_expr, "||", predicate_expr
               | "(", predicate_expr, ")" ;

// Custom elements: use attributes/classes with where predicates (e.g., heading.where(class == "my-macro"))

(* ===== Primitives ===== *)

literal      = int_lit | float_lit | string_lit | bool_lit | none_lit | symbol_lit ;
int_lit      = [ "-" ], digit, { digit } ;
float_lit    = [ "-" ], digit, { digit }, ".", digit, { digit } ;
string_lit   = '"', { string_char }, '"' ;
bool_lit     = "true" | "false" ;
none_lit     = "none" ;
symbol_lit   = "'", identifier ;

identifier   = ( letter | "_" ), { letter | digit | "_" | "-" } ;
binop        = "+" | "-" | "*" | "/" | "%" | "==" | "!=" | "<" | ">"
             | "<=" | ">=" | "&&" | "||" | "++" ;
unop         = "-" | "!" ;

params       = param, { ",", param } ;
param        = identifier, [ ":", type ], [ "=", expr ] ;

args         = arg, { ",", arg } ;
arg          = expr | identifier, ":", expr ;

type         = "Int" | "Float" | "String" | "Bool" | "None"
             | "Content" | "Block" | "Inline"
             | "Array" "<" type ">"
             | "Map" "<" type "," type ">"
             | "Code" "<" kind ">"
             | "Selector" "<" kind ">"
             | "Signal" "<" type ">" | "Effect"
             | "Dyn"
             | identifier ;

kind         = "Block" | "Inline" | "Content" ;
```

### 2.3 Markdown Coexistence Rules

MRL is designed to coexist with standard Markdown:

| Context | MRL Behavior |
|---------|--------------|
| Inside code fences | `!` is literal (no escape) |
| Inside inline code | `!` is literal |
| Inside HTML blocks | `!` is literal |
| After `!` in `![alt](url)` | Image syntax takes precedence |
| Escaped `!!` | Always produces literal `!` |

**Precedence**: Image syntax `![` binds tighter than MRL escape. To call a function named `[`, write `! [...]` with a space.

**Content literal vs. link**: Bare `[...]` in prose is parsed as Markdown (links if followed by `(url)`). `[...]` is only treated as an MRL content literal when inside `!` code (e.g., in `!def` bodies or `!name[...]`).

### 2.4 Examples

```markdown
# My Document

This is prose with an inline value: !version and a call: !today().

Here's a macro call with content:
!callout(severity: "warning")[
  Be careful with this operation!
]

!def greet(name: String):
  [Hello, *!name*!]

!staged[
  sections = doc.outline()
  for sec in sections:
    paragraph([Section: !sec.title at level !sec.level])
]

!set heading.where(level == 1) {
  numbering: "1.",
  font: "Georgia",
}

!show link.where(url.starts_with("http")):
  [!it.body ↗]

!live(deps: [slider_value])
  x = slider_value.get()
  plot(x ** 2, domain: (-10, 10))
```

---

## 3. Type and Kind System

### 3.1 Type Hierarchy

```
┌─────────────────────────────────────────────────────────────────┐
│                        TYPE HIERARCHY                           │
└─────────────────────────────────────────────────────────────────┘

  Type
  ├── Primitive
  │   ├── None
  │   ├── Bool
  │   ├── Int
  │   ├── Float
  │   ├── String
  │   └── Symbol
  │
  ├── Content                    (* document content *)
  │   ├── Block                  (* block-level content *)
  │   │   ├── Heading
  │   │   ├── Paragraph
  │   │   ├── CodeBlock
  │   │   ├── List
  │   │   ├── Blockquote
  │   │   ├── Table
  │   │   └── ThematicBreak
  │   │
  │   └── Inline                 (* inline content *)
  │       ├── Text
  │       ├── Emphasis
  │       ├── Strong
  │       ├── Code
  │       ├── Link
  │       ├── Image
  │       ├── Reference
  │       ├── Math
  │       └── Span
  │
  ├── Composite
  │   ├── Array<T>
  │   ├── Map<K, V>
  │   ├── Tuple<T...>
  │   └── Record { field: Type, ... }
  │
  ├── Function<(Args...) -> Return>
  │
  ├── Staged
  │   ├── Code<K>                (* quoted code producing K *)
  │   └── Shrubbery              (* raw syntax tree, macros only *)
  │
  ├── Selector<K>                (* element selector for show/set *)
  │
  ├── Reactive
  │   ├── Signal<T>              (* render-time reactive value *)
  │   └── Effect                 (* render-time side effect *)
  │
  └── Dyn                        (* runtime-typed escape hatch *)
```

### 3.2 Content Kinds

Content has a **kind** that determines where it can appear:

```
ContentKind ::= Block | Inline | Content

Subkinding:
  Block  <: Content
  Inline <: Content
```

**Key rule**: `Inline` cannot contain `Block`. This is enforced statically.

```
Γ ⊢ body : Inline
─────────────────────────  (Emphasis-Intro)
Γ ⊢ emphasis(body) : Inline

Γ ⊢ body : Content
───────────────────────────  (Blockquote-Intro)
Γ ⊢ blockquote(body) : Block
```

### 3.3 Staged Types: Code<K>

Following MetaOCaml, `Code<K>` represents a quoted expression that, when spliced/evaluated, produces a value of kind `K`.

```
Γ ⊢ e : K    K <: ContentKind
─────────────────────────────  (Quote)
   Γ ⊢ '[e] : Code<K>

Γ ⊢ e : Code<K>
────────────────────  (Splice)
   Γ ⊢ $e : K

Γ ⊢ e : Code<K>    (at expand-time)
────────────────────────────────────  (Eval)
      Γ ⊢ eval(e) : K
```

**Cross-stage persistence (CSP)**: Values from outer stages can be referenced in quoted code:

```
!def make_heading(level: Int): Code<Block> =
  '[ heading(level: $level, body: [Title]) ]
  // 'level' is CSP'd into the quoted code
```

### 3.4 Selector Types

Selectors are typed to ensure show/set rules are well-formed:

```
Selector<K> where K : ContentKind

heading           : Selector<Block>
paragraph         : Selector<Block>
emphasis          : Selector<Inline>
link              : Selector<Inline>
heading.where(p)  : Selector<Block>   // predicate doesn't change kind
```

### 3.5 The Dyn Boundary

Not all values can be statically typed. `Dyn` provides an escape hatch:

```
       Γ ⊢ e : T
─────────────────────────  (Inject)
   Γ ⊢ dyn(e) : Dyn

Γ ⊢ e : Dyn    (runtime check that e : T)
──────────────────────────────────────────  (Project)
          Γ ⊢ e as T : T
```

**Use cases for Dyn**:
- Values from external kernels (e.g., JS)
- JSON/YAML data
- Plugin return values
- User-provided configuration

### 3.6 Typing Rules for Staging

**Stage annotation**: Each expression has a stage level n ≥ 0.

```
Γ ⊢ₙ e : T    (expression e at stage n has type T)

Present stage (n = 0): executed at expand-time
Future stages (n > 0): inside quotes, executed at render-time or later
```

**Key rules**:

```
Γ ⊢ₙ e : T
──────────────────────  (Quote-Expr)
Γ ⊢ₙ '{e} : Code<T>

Γ ⊢ₙ e : Code<T>
──────────────────────  (Splice)
Γ ⊢ₙ₊₁ $(e) : T

Γ, x :ₙ T ⊢ₙ e : U
────────────────────────────────────  (Lambda)
Γ ⊢ₙ def(x: T): e : (T) -> U

x :ₘ T ∈ Γ    m ≤ n
────────────────────────  (Var)
     Γ ⊢ₙ x : T
```

**Cross-stage persistence**: A variable bound at stage m can be used at stage n if m ≤ n. When m < n, the value is "lifted" into the quoted code.

---

## 4. AST and Syntax Objects

### 4.1 Shrubbery Representation

Source is first parsed into a **shrubbery**—a token tree with grouping but deferred operator precedence:

```rust
enum Shrubbery {
    // Atoms
    Identifier(Symbol, ScopeSet, Span),
    Literal(Literal, Span),
    Operator(Symbol, Span),
    
    // Groups
    Parens(Vec<Shrubbery>, Span),      // (...)
    Brackets(Vec<Shrubbery>, Span),    // [...]
    Braces(Vec<Shrubbery>, Span),      // {...}
    
    // Content
    Prose(String, Span),               // raw text
    ContentBlock(Vec<Shrubbery>, Span), // [prose and !code]
    
    // Sequences
    Sequence(Vec<Shrubbery>, Span),    // comma or newline separated
}
```

### 4.2 Scope Sets for Hygiene

Following Racket's "binding as sets of scopes" model:

- Each binding form introduces a **scope** (unique token)
- Each identifier carries a **scope set**
- Binding resolution: find the binding whose scope set is a subset of the use site's scope set, with the largest such set

### 4.3 Macro Expansion and Hygiene

When a macro is invoked:

1. **Introduce macro scope**: Fresh scope M added to macro body
2. **Introduce use-site scope**: Fresh scope U added to macro arguments
3. **Expand**: Macro body executed, producing new shrubbery
4. **Flip scopes**: In output, M is flipped (removed if present, added if absent)

This ensures:
- Macro-introduced bindings have M, don't capture user bindings
- User-provided identifiers have U, aren't captured by macro bindings
- Explicit hygiene-breaking is possible but explicit

### 4.4 Enforestation

After macro expansion, **enforestation** resolves operator precedence to produce a typed AST.

---

## 5. Expand-Time Interpreter

### 5.1 Evaluation Model

Expand-time interpreter is deterministic and capability-gated. It evaluates expressions, executes `!staged` blocks, applies macros with hygiene, and produces `Content` plus compiled live-cell modules.

### 5.2 Determinism Requirements

Allowed: pure computation, document reflection, capability-granted file reads.  
Forbidden: nondeterministic sources (random, time), network I/O without capability, floating-point nondeterminism.

### 5.3 Document Reflection Built-ins

- `doc.outline() -> Array<OutlineEntry>`
- `doc.refs() -> Array<Reference>`
- `doc.find(selector) -> Array<Element>`
- `doc.meta(key) -> Option<Dyn>`
- `doc.here() -> SectionContext`

### 5.4 Show/Set Rule Evaluation

Show/set run at expand-time only. They operate on typed elements, not shrubbery. They cannot shell out or run staged code.
Show rules bind the matched element implicitly as `it` and must return the same kind (`K -> K`).

---

## 6. WASM Runtime ABI

Render-time code compiles to WASM and runs in the browser with:

- Reactive signal primitives
- UI widget constructors
- Diagnostic/decoration APIs
- Capability-gated fetch
- Dataspace client for actor communication

Render-time languages: **JS/WASM only**. No render-time Python.

Imports include signal creation/get/set, ui widgets (slider, text input, button), diagnostics/decorations, fetch, dataspace publish/subscribe, kernel eval (for JS/WASM kernels), and memory management.

---

## 7. Codegen Strategy

Recommend a **direct WASM emitter** for fast iteration and small per-cell modules. Interpreter fallback for trivial expressions. Alternative (Rust→wasm32) is heavier; JS is less isolated.

---

## 8. Standard Library

- Expand-time: reflection (`doc.*`), JSON/YAML (cap-gated), Content constructors, math/string/array/map helpers.
- Render-time: signals, ui widgets, fetch (cap-gated), diagnostics/decorations, kernel eval, dataspace ops.

---

## 9. Examples and Golden Tests

- Callout, figure, abbr macros
- Self-reflective outline generator with `!staged`
- Show/set rules targeting selectors
- Live interactive cell (render-time JS/WASM)
- No render-time Python examples

---

## 10. Security Model

Capability schema for expand-time (file/env/fetch) and render-time (fetch/ui/diagnostics/dataspace/kernel). WASM sandboxes per cell with memory/time limits. Deterministic expand-time; no render-time Python.

---

## 11. Open Questions and Risks

- Macro hygiene edge cases (hygiene breaks)
- Selector typing and predicate semantics (text vs Content)
- Error reporting across stages (span mapping)
- Incremental recompilation granularity and live-cell hot swap
- WASM performance vs interpreter fallback tuning

---

## 12. Implementation Roadmap

For up-to-date planning, see `vault/design/design.md` and `vault/sprints/`. This section is non-authoritative and kept minimal:
- Core parser + expand-time interpreter
- Macros + quote/splice + hygiene
- Show/set + selectors
- WASM codegen + runtime ABI
- Integration with queries/CRDT/dataspaces
- Polish: errors, perf, tests, docs

---
