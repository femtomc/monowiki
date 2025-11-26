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

#### Quote/Splice Semantics

**Quote** (`'[...]` or `quote:`) creates a `Code<K>` value:
- The expression inside is NOT evaluated at the current stage
- It represents code that will produce a value of kind K when evaluated later
- Type rule: If `e : K` then `'[e] : Code<K>`

**Splice** (`$expr` or `splice(expr)`) extracts code from `Code<K>`:
- Only valid inside a quoted context
- Evaluates `expr` at the OUTER stage, inserts the result into the inner quoted code
- Type rule: If `e : Code<K>` then inside a quote, `$e : K`

**Formal typing rules**:

```
Γ ⊢ e : K    K <: ContentKind
─────────────────────────────  (Quote)
   Γ ⊢ '[e] : Code<K>

Γ ⊢ e : Code<K>    (inside quote context)
────────────────────────────────────────  (Splice)
   Γ ⊢ $e : K

Γ ⊢ e : Code<K>    (at expand-time)
────────────────────────────────────  (Eval)
      Γ ⊢ eval(e) : K
```

#### Cross-Stage Persistence (CSP)

When a value from an outer stage is referenced in a quote, it is automatically "persisted" into the quoted code. This is automatic for serializable values.

**Example**:
```
!def make_heading(level: Int) -> Code<Block>:
  '[ heading(level: $level, body: [Title]) ]
  // 'level' (Int) is CSP'd: its VALUE is captured, not its binding
```

At expand-time when `make_heading(2)` is called:
1. `level` is bound to `2` in the outer stage
2. Inside the quote, `$level` splices the value `2`
3. The resulting `Code<Block>` represents: `heading(level: 2, body: [Title])`

**Important**: CSP captures VALUES, not variable bindings. If you modify a variable after creating a quote, the quote still contains the original value:

```
!def example():
  x = 1
  q1 = '[ text(str($x)) ]  // Captures value 1
  x = 2
  q2 = '[ text(str($x)) ]  // Captures value 2
  // q1 still represents text("1"), not text("2")
```

#### The `it` Binding in Show Rules

Show rules bind the matched element as `it`. This is NOT hygiene-breaking because `it` is a well-known, documented binding convention, not a user-defined identifier.

**Example**:
```
!show heading.where(level == 1):
  // 'it' refers to the matched heading element
  // Type: it : Block (specifically, the Heading variant)
  page_break() + it

!show link.where(url.starts_with("http")):
  // 'it' refers to the matched link element
  // Type: it : Inline (specifically, the Link variant)
  link(body: it.body + text(" ↗"), url: it.url)
```

**Type of `it`**: The type of `it` depends on the selector:
- `heading` → `it : Block` (Heading variant)
- `paragraph` → `it : Block` (Paragraph variant)
- `link` → `it : Inline` (Link variant)
- `emphasis` → `it : Inline` (Emphasis variant)
- `heading.where(level == 1)` → `it : Block` (still Heading, but filtered)

**Constraint**: Show rules must be type-preserving. If the selector has type `Selector<K>`, the transform must have type `K -> K`. This ensures:
- A show rule for `heading` (which is `Block`) must return a `Block`
- A show rule for `link` (which is `Inline`) must return an `Inline`
- You cannot return an `Inline` from a `Block` selector or vice versa

#### Stage Levels and Nested Quotes

Each expression has a stage level `n ≥ 0`:

```
Γ ⊢ₙ e : T    (expression e at stage n has type T)

Stage 0 (present): Executed at expand-time
Stage 1: Inside one level of quotes, executed at render-time
Stage 2+: Inside nested quotes
```

**Key rules for stage levels**:

```
Γ ⊢ₙ e : T
──────────────────────  (Quote-Level)
Γ ⊢ₙ '{e} : Code<T>

Γ ⊢ₙ e : Code<T>
──────────────────────  (Splice-Level)
Γ ⊢ₙ₊₁ $(e) : T

x :ₘ T ∈ Γ    m ≤ n
────────────────────────  (Var-CSP)
     Γ ⊢ₙ x : T
```

The `Var-CSP` rule allows a variable bound at stage `m` to be used at stage `n` if `m ≤ n`. This is the formal basis for cross-stage persistence.

**Example with nested quotes**:
```
!def make_macro(name: String) -> Code<Code<Block>>:
  quote:
    quote:
      heading(2, text($name))
      // Here, $name splices from stage 0 into stage 2
      // The outer quote is at stage 1
```

This is rarely needed in practice but demonstrates the staging discipline.

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

### 8.1 Expand-Time Functions

#### 8.1.1 Document Reflection

| Function | Type | Description |
|----------|------|-------------|
| `doc.outline()` | `() -> Array<OutlineEntry>` | Get document heading structure with level, title, id |
| `doc.refs()` | `() -> Array<Reference>` | Get all cross-references in document |
| `doc.find(selector)` | `Selector<K> -> Array<K>` | Find all elements matching selector |
| `doc.meta(key)` | `String -> Option<Dyn>` | Get frontmatter metadata value |
| `doc.here()` | `() -> SectionContext` | Current section context (parent heading, depth) |

**Types:**
```
OutlineEntry = {
  level: Int,
  title: Inline,
  id: String,
  children: Array<OutlineEntry>,
}

Reference = {
  id: String,
  target: Option<Element>,
  source_span: Span,
}

SectionContext = {
  heading: Option<Heading>,
  depth: Int,
  path: Array<String>,
}
```

#### 8.1.2 Content Constructors

| Function | Type | Description |
|----------|------|-------------|
| `heading(level, body)` | `(Int, Inline) -> Block` | Create heading (level 1-6) |
| `paragraph(body)` | `Inline -> Block` | Create paragraph block |
| `text(s)` | `String -> Inline` | Create text node |
| `emphasis(body)` | `Inline -> Inline` | Wrap in emphasis (italic) |
| `strong(body)` | `Inline -> Inline` | Wrap in strong (bold) |
| `link(body, url, title?)` | `(Inline, String, String?) -> Inline` | Create hyperlink |
| `image(alt, url, title?)` | `(String, String, String?) -> Inline` | Create image |
| `code_block(lang?, code)` | `(String?, String) -> Block` | Create code block |
| `code(value)` | `String -> Inline` | Create inline code span |
| `list(items)` | `Array<ListItem> -> Block` | Create list block |
| `blockquote(body)` | `Content -> Block` | Create blockquote |
| `span(body, attrs?)` | `(Inline, Map<String, String>?) -> Inline` | Create span with attributes |
| `directive(tag, attrs?, body?)` | `(String, Map?, Content?) -> Block` | Create generic directive element |

**Note**: `directive` is the low-level primitive for custom elements. Macros typically use it to generate custom HTML-like structures.

#### 8.1.3 Data Functions (Capability-Gated)

| Function | Type | Description |
|----------|------|-------------|
| `load_json(path)` | `String -> Dyn` | Load and parse JSON file (requires filesystem cap) |
| `load_yaml(path)` | `String -> Dyn` | Load and parse YAML file (requires filesystem cap) |
| `env(key)` | `String -> Option<String>` | Get environment variable (requires env cap) |
| `read_file(path)` | `String -> String` | Read file as string (requires filesystem cap) |

**Security**: These functions require explicit capability grants. Documents without the appropriate capabilities will get a compile-time error if they attempt to use these functions.

#### 8.1.4 String Functions

| Function | Type | Description |
|----------|------|-------------|
| `str.len(s)` | `String -> Int` | String length in characters |
| `str.split(s, sep)` | `(String, String) -> Array<String>` | Split string by separator |
| `str.join(arr, sep)` | `(Array<String>, String) -> String` | Join strings with separator |
| `str.starts_with(s, prefix)` | `(String, String) -> Bool` | Check if string starts with prefix |
| `str.ends_with(s, suffix)` | `(String, String) -> Bool` | Check if string ends with suffix |
| `str.contains(s, substr)` | `(String, String) -> Bool` | Check if string contains substring |
| `str.replace(s, old, new)` | `(String, String, String) -> String` | Replace all occurrences |
| `str.trim(s)` | `String -> String` | Remove leading/trailing whitespace |
| `str.upper(s)` | `String -> String` | Convert to uppercase |
| `str.lower(s)` | `String -> String` | Convert to lowercase |

#### 8.1.5 Array Functions

| Function | Type | Description |
|----------|------|-------------|
| `arr.len(a)` | `Array<T> -> Int` | Array length |
| `arr.map(a, f)` | `(Array<T>, T -> U) -> Array<U>` | Map function over array |
| `arr.filter(a, p)` | `(Array<T>, T -> Bool) -> Array<T>` | Filter array by predicate |
| `arr.fold(a, init, f)` | `(Array<T>, U, (U, T) -> U) -> U` | Fold (reduce) array |
| `arr.find(a, p)` | `(Array<T>, T -> Bool) -> Option<T>` | Find first matching element |
| `arr.any(a, p)` | `(Array<T>, T -> Bool) -> Bool` | Check if any element matches |
| `arr.all(a, p)` | `(Array<T>, T -> Bool) -> Bool` | Check if all elements match |
| `arr.sort(a, cmp?)` | `(Array<T>, (T, T) -> Int)? -> Array<T>` | Sort array |
| `arr.reverse(a)` | `Array<T> -> Array<T>` | Reverse array |
| `arr.concat(a, b)` | `(Array<T>, Array<T>) -> Array<T>` | Concatenate arrays |
| `arr.slice(a, start, end?)` | `(Array<T>, Int, Int?) -> Array<T>` | Slice array |

#### 8.1.6 Map Functions

| Function | Type | Description |
|----------|------|-------------|
| `map.get(m, k)` | `(Map<K,V>, K) -> Option<V>` | Get value for key |
| `map.set(m, k, v)` | `(Map<K,V>, K, V) -> Map<K,V>` | Set key-value pair (pure) |
| `map.has(m, k)` | `(Map<K,V>, K) -> Bool` | Check if key exists |
| `map.delete(m, k)` | `(Map<K,V>, K) -> Map<K,V>` | Delete key (pure) |
| `map.keys(m)` | `Map<K,V> -> Array<K>` | Get all keys |
| `map.values(m)` | `Map<K,V> -> Array<V>` | Get all values |
| `map.entries(m)` | `Map<K,V> -> Array<Tuple<K,V>>` | Get key-value pairs |
| `map.merge(m1, m2)` | `(Map<K,V>, Map<K,V>) -> Map<K,V>` | Merge maps (m2 overwrites) |

#### 8.1.7 Math Functions

| Function | Type | Description |
|----------|------|-------------|
| `math.abs(x)` | `Float -> Float` | Absolute value |
| `math.floor(x)` | `Float -> Int` | Round down |
| `math.ceil(x)` | `Float -> Int` | Round up |
| `math.round(x)` | `Float -> Int` | Round to nearest |
| `math.min(x, y)` | `(Float, Float) -> Float` | Minimum |
| `math.max(x, y)` | `(Float, Float) -> Float` | Maximum |
| `math.pow(x, y)` | `(Float, Float) -> Float` | Power |
| `math.sqrt(x)` | `Float -> Float` | Square root |
| `math.sin(x)` | `Float -> Float` | Sine |
| `math.cos(x)` | `Float -> Float` | Cosine |
| `math.tan(x)` | `Float -> Float` | Tangent |
| `math.log(x)` | `Float -> Float` | Natural logarithm |
| `math.exp(x)` | `Float -> Float` | Exponential |

**Constants**: `math.pi`, `math.e`

### 8.2 Render-Time Functions

**Note**: Render-time code runs in the browser as JS/WASM. These functions are available in `!live` blocks only.

#### 8.2.1 Reactive Signals

| Function | Type | Description |
|----------|------|-------------|
| `signal(initial)` | `T -> Signal<T>` | Create reactive signal with initial value |
| `signal.get(s)` | `Signal<T> -> T` | Get current signal value |
| `signal.set(s, v)` | `(Signal<T>, T) -> ()` | Set signal value (triggers reactivity) |
| `signal.update(s, f)` | `(Signal<T>, T -> T) -> ()` | Update signal with function |
| `signal.subscribe(s, callback)` | `(Signal<T>, T -> ()) -> SubId` | Subscribe to signal changes |
| `signal.unsubscribe(id)` | `SubId -> ()` | Unsubscribe from signal |

**Example**:
```
!live
  count = signal(0)

  def increment():
    signal.update(count, x -> x + 1)

  signal.subscribe(count, value ->
    ui.show(text("Count: " + str(value)))
  )
```

#### 8.2.2 UI Widgets

| Function | Type | Description |
|----------|------|-------------|
| `ui.slider(min, max, opts?)` | `(Float, Float, SliderOpts?) -> Signal<Float>` | Create slider widget |
| `ui.text_input(opts?)` | `TextInputOpts? -> Signal<String>` | Create text input |
| `ui.number_input(opts?)` | `NumberInputOpts? -> Signal<Float>` | Create number input |
| `ui.checkbox(opts?)` | `CheckboxOpts? -> Signal<Bool>` | Create checkbox |
| `ui.select(options, opts?)` | `(Array<String>, SelectOpts?) -> Signal<String>` | Create dropdown select |
| `ui.button(label, onclick)` | `(String, () -> ()) -> Widget` | Create button |
| `ui.show(value)` | `Dyn -> ()` | Display value in output area |
| `ui.clear()` | `() -> ()` | Clear output area |

**Option types**:
```
SliderOpts = {
  value?: Float,
  step?: Float,
  label?: String,
}

TextInputOpts = {
  value?: String,
  placeholder?: String,
  multiline?: Bool,
}

NumberInputOpts = {
  value?: Float,
  step?: Float,
  min?: Float,
  max?: Float,
}

CheckboxOpts = {
  checked?: Bool,
  label?: String,
}

SelectOpts = {
  value?: String,
  label?: String,
}
```

#### 8.2.3 Diagnostics and Decorations

| Function | Type | Description |
|----------|------|-------------|
| `diag.error(span, msg)` | `(Span, String) -> ()` | Emit error diagnostic |
| `diag.warn(span, msg)` | `(Span, String) -> ()` | Emit warning diagnostic |
| `diag.info(span, msg)` | `(Span, String) -> ()` | Emit info diagnostic |
| `diag.hint(span, msg)` | `(Span, String) -> ()` | Emit hint diagnostic |
| `diag.decorate(span, class, attrs?)` | `(Span, String, Map?) -> ()` | Add decoration to text range |
| `diag.clear()` | `() -> ()` | Clear all diagnostics from this cell |

**Example**:
```
!live
  code = ui.text_input(value: "let x = 42")

  def lint():
    if code.get().contains("var"):
      diag.warn(span(0, 3), "Prefer 'let' over 'var'")

  code.subscribe(lint)
```

#### 8.2.4 Fetch (Capability-Gated)

| Function | Type | Description |
|----------|------|-------------|
| `fetch(url, opts?)` | `(String, FetchOpts?) -> Promise<Response>` | HTTP fetch (requires network cap) |

**Types**:
```
FetchOpts = {
  method?: String,        // "GET", "POST", etc.
  headers?: Map<String, String>,
  body?: String,
  timeout?: Int,          // milliseconds
}

Response = {
  status: Int,
  headers: Map<String, String>,
  text: () -> Promise<String>,
  json: () -> Promise<Dyn>,
}
```

**Example**:
```
!live
  def load_data():
    response = await fetch("https://api.example.com/data")
    data = await response.json()
    ui.show(data)

  ui.button("Load Data", load_data)
```

#### 8.2.5 Dataspace Operations

| Function | Type | Description |
|----------|------|-------------|
| `ds.publish(pattern, value)` | `(String, Dyn) -> AssertionId` | Publish assertion to dataspace |
| `ds.retract(id)` | `AssertionId -> ()` | Retract assertion |
| `ds.subscribe(pattern, callback)` | `(String, Dyn -> ()) -> SubId` | Subscribe to pattern in dataspace |
| `ds.unsubscribe(id)` | `SubId -> ()` | Unsubscribe from pattern |
| `ds.query(pattern)` | `String -> Array<Dyn>` | One-time query for matching assertions |

**Note**: Dataspace operations require appropriate capability grants. Patterns follow the syndicated actors pattern matching syntax.

#### 8.2.6 Plotting (Optional Extension)

| Function | Type | Description |
|----------|------|-------------|
| `plot.line(data, opts?)` | `(Array<Point>, PlotOpts?) -> Widget` | Line plot |
| `plot.scatter(data, opts?)` | `(Array<Point>, PlotOpts?) -> Widget` | Scatter plot |
| `plot.bar(data, opts?)` | `(Array<BarData>, PlotOpts?) -> Widget` | Bar chart |
| `plot.histogram(data, opts?)` | `(Array<Float>, HistOpts?) -> Widget` | Histogram |

**Note**: Plotting functions are part of an optional standard library extension and may not be available in all environments.

---

## 9. Examples and Golden Tests

This section provides complete, runnable examples demonstrating MRL features. Each example is intended to serve as a golden test case.

### 9.1 Callout Macro (Block-Level)

```
!def callout(severity: Symbol = 'info, body: Shrubbery) -> Code<Block>:
  quote:
    let class_map = {
      'note: "callout-note",
      'warn: "callout-warn",
      'error: "callout-error",
      'tip: "callout-tip",
    }
    let cls = map.get(class_map, $severity)
    let default_cls = "callout-info"
    let final_cls = if cls != none: cls else: default_cls

    directive(
      "div",
      attrs: {"class": final_cls},
      body: $expand(body)
    )

// Usage:
!callout(severity: 'warn)[
  Be careful when using this feature! It may have unexpected side effects.
]

!callout[
  This uses the default severity level ('info).
]

// Expands to (conceptually):
// <div class="callout-warn">
//   <p>Be careful when using this feature! It may have unexpected side effects.</p>
// </div>
```

### 9.2 Figure Macro with Caption

```
!def figure(src: String, caption: Shrubbery, name: String = "") -> Code<Block>:
  quote:
    let id_attrs = if $name != "":
      {"id": $name}
    else:
      {}

    directive(
      "figure",
      attrs: id_attrs,
      body:
        image(alt: "", url: $src) +
        directive("figcaption", body: $expand(caption))
    )

// Usage:
!figure(src: "architecture.png", name: "fig-arch")[
  The system architecture showing the three-phase pipeline:
  read-time, expand-time, and render-time.
]

// Later reference:
See @fig-arch for details.

// Expands to:
// <figure id="fig-arch">
//   <img src="architecture.png" alt="" />
//   <figcaption>
//     The system architecture showing the three-phase pipeline:
//     read-time, expand-time, and render-time.
//   </figcaption>
// </figure>
```

### 9.3 Self-Reflective Table of Contents

```
!staged[
  let outline = doc.outline()

  heading(2, text("Table of Contents"))

  for entry in outline:
    let indent = str.join(arr.map(arr.slice([0], 0, entry.level - 1), _ -> "  "), "")
    let num = str(entry.level) + ". "
    paragraph(
      text(indent + num) +
      link(entry.title, "#" + entry.id)
    )
]

// This generates a complete table of contents at expand-time,
// based on all headings in the document. If headings change,
// the ToC is automatically regenerated.
```

### 9.4 Show Rule for External Links

```
!show link.where(url.starts_with("http")):
  // 'it' is the matched link element
  link(
    body: it.body + text(" \u2197"),  // Arrow up-right symbol
    url: it.url,
    title: it.title
  )

// Now all external links automatically get an arrow:
Visit [our website](https://example.com) for more information.
// Renders as: Visit our website ↗ for more information.

// Internal links are unaffected:
See @sec-intro for background.
```

### 9.5 Show Rule with Conditional Styling

```
!show heading.where(level == 1):
  // Add page break before top-level headings
  thematic_break() + it

!show heading.where(level >= 2):
  // Add section symbol before subsection headings
  heading(
    level: it.level,
    body: text("\u00A7 ") + it.body  // Section symbol
  )

// First heading:
# Introduction
// Renders with page break before

## Background
// Renders as: § Background

### Details
// Renders as: § Details
```

### 9.6 Set Rules for Document Styling

```
!set heading {
  numbering: "1.1",
  font: "Georgia",
}

!set heading.where(level == 1) {
  page_break_before: true,
  font_size: 24,
}

!set code_block {
  theme: "github-dark",
  line_numbers: true,
  font: "JetBrains Mono",
}

!set paragraph {
  line_height: 1.6,
  text_align: "justify",
}

// These style rules apply to all matching elements in the document
```

### 9.7 Interactive Quadratic Explorer (Render-Time)

```
!live
  a_sig = ui.slider(-5, 5, value: 1, step: 0.1, label: "a")
  b_sig = ui.slider(-5, 5, value: 0, step: 0.1, label: "b")
  c_sig = ui.slider(-5, 5, value: 0, step: 0.1, label: "c")

  def update():
    a = a_sig.get()
    b = b_sig.get()
    c = c_sig.get()

    // Generate points for the parabola
    points = []
    for i in range(-10, 11):
      x = i / 2.0
      y = a * x * x + b * x + c
      arr.push(points, {x: x, y: y})

    // Display the equation
    ui.show(text("f(x) = " + str(a) + "x² + " + str(b) + "x + " + str(c)))

    // Plot the function
    ui.show(plot.line(points, {
      x_label: "x",
      y_label: "f(x)",
      title: "Quadratic Function",
    }))

  // Subscribe to all sliders
  a_sig.subscribe(update)
  b_sig.subscribe(update)
  c_sig.subscribe(update)

  // Initial render
  update()
```

### 9.8 Data-Driven Content Generation

```
!staged[
  let team_data = load_json("team.json")
  // team.json structure:
  // {
  //   "members": [
  //     {"name": "Alice", "role": "Lead", "bio": "...", "links": [...]},
  //     ...
  //   ]
  // }

  heading(2, text("Team Members"))

  for person in team_data.members:
    heading(3, text(person.name + " - " + person.role))
    paragraph(text(person.bio))

    if person.links && arr.len(person.links) > 0:
      list([
        {
          content: link(text(l.name), l.url),
          checked: false,
        }
        for l in person.links
      ])
]
```

### 9.9 Abbreviation Macro (Inline)

```
!def abbr(title: String, body: Shrubbery) -> Code<Inline>:
  quote:
    span(
      $expand(body),
      attrs: {
        "title": $title,
        "class": "abbr",
        "style": "text-decoration: underline dotted;",
      }
    )

// Usage:
Modern web pages are written in !abbr(title: "HyperText Markup Language")[HTML],
styled with !abbr(title: "Cascading Style Sheets")[CSS], and made interactive
with !abbr(title: "JavaScript")[JS].

// When rendered, hovering over HTML/CSS/JS shows the full name in a tooltip.
```

### 9.10 Conditional Content Based on Metadata

```
!staged[
  let draft_mode = doc.meta("draft")

  if draft_mode == true:
    callout(severity: 'warn)[
      This document is in DRAFT mode. Content may be incomplete or inaccurate.
    ]

  let author = doc.meta("author")
  let date = doc.meta("date")

  if author:
    paragraph(strong(text("Author: ")) + text(author))

  if date:
    paragraph(strong(text("Date: ")) + text(date))
]

// Frontmatter:
// ---
// draft: true
// author: "Jane Doe"
// date: "2024-03-15"
// ---
```

### 9.11 Theorem Environment (Mathematical Writing)

```
!def theorem(title: String, body: Shrubbery) -> Code<Block>:
  quote:
    let here_ctx = doc.here()
    let theorem_num = str(here_ctx.depth) + ".1"  // Simplified numbering

    directive(
      "div",
      attrs: {
        "class": "theorem-box",
        "style": "border-left: 3px solid blue; padding: 1em; margin: 1em 0;",
      },
      body:
        paragraph(
          strong(text("Theorem " + theorem_num)) +
          if $title != "": text(" (" + $title + ")") else: text("") +
          text(". ")
        ) +
        $expand(body)
    )

// Usage:
!theorem(title: "Fundamental Theorem of Calculus")[
  If $f$ is continuous on $[a, b]$ and $F$ is an antiderivative of $f$,
  then:

  $$\int_a^b f(x)\,dx = F(b) - F(a)$$
]
```

### 9.12 Bibliography Entry Generator

```
!staged[
  let bibliography = load_yaml("references.yaml")
  // references.yaml structure:
  // - id: knuth1984
  //   author: "Donald Knuth"
  //   title: "The TeXbook"
  //   year: 1984
  //   publisher: "Addison-Wesley"

  heading(2, text("References"))

  for entry in bibliography:
    paragraph(
      text("[" + entry.id + "] ") +
      strong(text(entry.author)) +
      text(". ") +
      emphasis(text(entry.title)) +
      text(". ") +
      text(entry.publisher + ", " + str(entry.year) + ".")
    )
]
```

### 9.13 Live API Data Fetcher

```
!live
  status_signal = signal("idle")
  data_signal = signal(none)

  def fetch_data():
    status_signal.set("loading")
    ui.show(text("Loading..."))

    try:
      response = await fetch("https://api.github.com/repos/rust-lang/rust")
      data = await response.json()
      data_signal.set(data)
      status_signal.set("success")

      ui.clear()
      ui.show(strong(text("Repository: ")) + text(data.full_name))
      ui.show(text("Stars: " + str(data.stargazers_count)))
      ui.show(text("Forks: " + str(data.forks_count)))
      ui.show(text("Open Issues: " + str(data.open_issues_count)))
    catch err:
      status_signal.set("error")
      ui.show(text("Error: " + str(err)))

  ui.button("Fetch Repository Data", fetch_data)
```

### 9.14 Custom Selector with Attribute Matching

```
!show span.where(class == "highlight"):
  span(
    it.body,
    attrs: {
      "class": "highlight",
      "style": "background-color: yellow; padding: 0.2em;",
    }
  )

// Usage:
This is [important text]{.highlight} that needs emphasis.

// The show rule adds styling to all spans with class="highlight"
```

### 9.15 Glossary Generator with Cross-References

```
!staged[
  let glossary_terms = {
    "CRDT": "Conflict-free Replicated Data Type",
    "WASM": "WebAssembly",
    "AST": "Abstract Syntax Tree",
  }

  heading(2, text("Glossary"))

  for key in map.keys(glossary_terms):
    let value = map.get(glossary_terms, key)
    paragraph(
      strong(text(key)) +
      text(": ") +
      text(value)
    )
]

// Elsewhere in the document:
// The document uses a !abbr(title: map.get(glossary_terms, "CRDT"))[CRDT]
// to enable collaboration.
```

**Note**: These examples demonstrate the full range of MRL features from simple macros to complex interactive applications. Each should be testable in the implementation.

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

## 12. FAQ

### Q: Why use `!` instead of `@` or `#` for the language escape?

**A**: The `!` character was chosen for several reasons:
- It's visually distinct and naturally suggests "action" or "evaluation"
- It doesn't conflict with Markdown image syntax (where `!` only matters before `[`)
- It's easy to type on all keyboard layouts
- It's not commonly used in prose (unlike `@` which appears in email addresses)
- The double-bang `!!` escape for literal `!` is intuitive

### Q: Can I use Python in render-time code?

**A**: No. Render-time code (`!live` blocks) runs in the browser as JS/WASM. Python would require shipping a Python runtime to the browser, which is impractical for size and performance reasons.

**Alternatives**:
- Use expand-time `!staged` blocks for Python computation (runs once during compilation)
- Use a server-side kernel architecture (planned future feature)
- Transpile Python to JS/WASM (not officially supported)

The distinction is intentional: expand-time is deterministic and can run arbitrary languages; render-time must be fast, sandboxed, and browser-compatible.

### Q: How do I make a hygiene-breaking macro?

**A**: Hygiene-breaking is intentionally difficult to discourage its use, but it's possible when necessary:

```
!def unhygienic_macro(var_name: String, body: Shrubbery) -> Code<Block>:
  let introduced_id = syntax.introduce(macro_scope, var_name)
  quote:
    let $introduced_id = "macro-injected value"
    $expand(body)

// This macro CAN capture user bindings if var_name matches
```

Use `syntax.introduce(scope, identifier)` to explicitly introduce an identifier from a specific scope. This makes the hygiene break visible in the macro definition.

**Best practice**: Document hygiene-breaking macros clearly and use unique prefixes for introduced bindings (e.g., `__macro_temp`) to minimize capture risk.

### Q: What's the difference between `!staged` and `!live`?

**A**: The key difference is **when** the code runs and **how** it integrates into the document:

| Feature | `!staged` | `!live` |
|---------|-----------|---------|
| **Runs when** | Expand-time (compilation) | Render-time (browser) |
| **Runs how often** | Once, when document is built | Continuously, responds to user input |
| **Output** | Baked into `Content` tree | Dynamic DOM updates |
| **Can access** | Document reflection, file I/O | UI widgets, signals, fetch |
| **Language** | Any (Python, Rust, etc.) | JS/WASM only |
| **Example use** | Generate ToC, load data | Interactive plots, calculators |

**Rule of thumb**: If it should happen once and be saved, use `!staged`. If it should react to user input, use `!live`.

### Q: Can show rules create new syntax?

**A**: No. Show rules transform `Content` (the typed AST), not `Shrubbery` (the syntax tree). They cannot define new syntactic forms.

**What show rules CAN do**:
- Transform how existing elements render
- Add attributes or styling
- Wrap elements in containers
- Filter or conditionally display elements

**What show rules CANNOT do**:
- Define new syntax (e.g., a new bracket type)
- Change parsing behavior
- Access the token stream

**For new syntax**, use macros (`!def`), which operate on `Shrubbery` and can define arbitrary syntactic extensions.

### Q: How do selectors for custom elements work?

**A**: Custom elements created by macros don't have built-in selectors like `heading` or `link`. Instead, target them using attribute selectors:

```
!def my_custom_box(content: Shrubbery) -> Code<Block>:
  quote:
    directive("div", attrs: {"class": "my-custom-box"}, body: $expand(content))

// Show rule targeting the custom element:
!show span.where(class == "my-custom-box"):
  // Apply custom styling
  directive("div", attrs: {
    "class": "my-custom-box",
    "style": "border: 2px solid blue;",
  }, body: it.body)
```

Alternatively, add a custom data attribute:
```
!def callout(body: Shrubbery) -> Code<Block>:
  quote:
    directive("div", attrs: {"data-macro": "callout"}, body: $expand(body))

!show div.where(data-macro == "callout"):
  // Style all callouts
```

### Q: Why can't I modify a signal in expand-time code?

**A**: Signals (`Signal<T>`) only exist at render-time. Expand-time code runs once during compilation and produces static `Content`. There's no "live" environment to hold signal state.

**Mental model**:
- Expand-time = compilation, static output
- Render-time = execution, dynamic behavior

If you need dynamic behavior at expand-time (e.g., re-run when a file changes), use the incremental computation system's dependency tracking. Queries are automatically recomputed when inputs change.

### Q: How do I debug macro expansion?

**A**: Several strategies:

1. **Use `diag.info()` to log intermediate values**:
```
!def debug_macro(x: Int) -> Code<Block>:
  diag.info(span.here(), "Macro called with x = " + str(x))
  quote:
    paragraph(text("Value: " + str($x)))
```

2. **Check the expanded `Content` tree**: Most implementations provide a `--dump-ast` flag to show the post-expansion tree.

3. **Use `quote`/`eval` boundaries explicitly** to control staging:
```
!def test():
  let intermediate = some_computation()
  diag.info(span.here(), "Intermediate: " + str(intermediate))
  quote:
    paragraph(text($intermediate))
```

4. **Write unit tests for macros** using the golden test framework (§9).

### Q: What happens if two show rules match the same element?

**A**: Show rules are applied in **definition order**. Later rules can see the output of earlier rules.

```
!show heading:
  heading(level: it.level, body: text("A: ") + it.body)

!show heading:
  heading(level: it.level, body: text("B: ") + it.body)

# Test
// Renders as: "B: A: Test"
```

**Composition**: Each rule receives the output of the previous rule as its input. This allows chaining transformations.

**Override**: To completely replace earlier rules, use `!set` instead, which defines defaults rather than transforms.

### Q: Can I use MRL in a non-collaborative setting?

**A**: Yes! The CRDT layer is optional. For single-user or version-control-based workflows:

- Disable real-time sync
- Store documents as plain text files
- Use Git for version control
- All MRL features (macros, staging, show/set) work identically

The CRDT is only activated when collaboration features are enabled.

### Q: How do I share macros across documents?

**A**: Several approaches:

1. **Import from a shared file** (planned):
```
!import macros from "shared/macros.mrl"
```

2. **Copy-paste for now**: Until imports are implemented, copy macro definitions to each document.

3. **Workspace-level macros** (planned): Define macros in a workspace configuration file that's automatically available to all documents.

4. **Standard library**: Common macros (callouts, figures, etc.) will be included in the standard library.

### Q: What's the performance overhead of incremental computation?

**A**: The query system adds overhead (dependency tracking, hashing), but for typical documents:

- **Edit-to-render latency**: < 16ms (one frame) for local edits
- **Memory overhead**: ~2-3x compared to non-incremental (due to caching)
- **Cache hit ratio**: > 95% for typical editing patterns

**Trade-off**: The overhead is worthwhile because:
- Recomputing everything on each edit is too slow for interactive editing
- Early cutoff prevents cascading recomputation
- Content-addressable caching enables cross-session speedups

### Q: Can I export to formats other than HTML?

**A**: Yes, via export pipelines (planned):

- **PDF**: Via HTML → print CSS → PDF
- **LaTeX**: Via `Content` → LaTeX AST → `.tex`
- **Markdown**: Via `Content` → CommonMark (lossy for custom elements)
- **DOCX**: Via HTML → Pandoc → DOCX (experimental)

Export is a render-time concern, so `!live` blocks may not export correctly. Use `!staged` for content that should appear in all export formats.

---

## 13. Appendix: Complete Grammar Reference

### A.1 Lexical Grammar

```
IDENTIFIER = [a-zA-Z_][a-zA-Z0-9_-]*
INT_LIT    = -?[0-9]+
FLOAT_LIT  = -?[0-9]+\.[0-9]+([eE][+-]?[0-9]+)?
STRING_LIT = "([^"\\]|\\.)*"
SYMBOL_LIT = '[a-zA-Z_][a-zA-Z0-9_-]*
OPERATOR   = + | - | * | / | % | == | != | < | > | <= | >= | && | || | ++ | !
COMMENT    = // [^\n]* | /* ([^*] | \*[^/])* */
```

### A.2 Reserved Words

```
Reserved keywords (cannot be used as identifiers):
  def, if, else, for, in, while, break, continue,
  quote, splice, eval, expand,
  true, false, none,
  staged, live, show, set, where,
  and, or, not,
  import, from, as,
  let, mut, const,
  match, case,
  try, catch, finally, throw,
  async, await,
  return, yield
```

### A.3 Operator Precedence (Highest to Lowest)

```
1. Function call, subscript, field access: f(), a[i], a.b
2. Unary: -, !, not
3. Exponentiation: ** (right-associative)
4. Multiplicative: *, /, %
5. Additive: +, -, ++
6. Comparison: <, >, <=, >=
7. Equality: ==, !=
8. Logical AND: &&, and
9. Logical OR: ||, or
10. Assignment: =, +=, -=, *=, /= (right-associative)
```

### A.4 Type Grammar

```ebnf
type = primitive_type
     | content_type
     | composite_type
     | function_type
     | staged_type
     | selector_type
     | reactive_type
     | "Dyn" ;

primitive_type = "None" | "Bool" | "Int" | "Float" | "String" | "Symbol" ;

content_type = "Content" | "Block" | "Inline" ;

composite_type = "Array" "<" type ">"
               | "Map" "<" type "," type ">"
               | "Tuple" "<" type { "," type } ">"
               | "Record" "{" field_types "}" ;

field_types = identifier ":" type { "," identifier ":" type } ;

function_type = "(" [ param_types ] ")" "->" type ;
param_types = type { "," type } ;

staged_type = "Code" "<" content_kind ">"
            | "Shrubbery" ;

content_kind = "Block" | "Inline" | "Content" ;

selector_type = "Selector" "<" content_kind ">" ;

reactive_type = "Signal" "<" type ">"
              | "Effect" ;
```

---

## 14. Implementation Roadmap

For up-to-date planning, see `vault/design/design.md` and `vault/sprints/`. This section is non-authoritative and kept minimal:
- Core parser + expand-time interpreter
- Macros + quote/splice + hygiene
- Show/set + selectors
- WASM codegen + runtime ABI
- Integration with queries/CRDT/dataspaces
- Polish: errors, perf, tests, docs

---
