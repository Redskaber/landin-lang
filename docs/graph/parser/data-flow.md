# Parser Data Flow (tokens → AST)

> **Date**: 2026-08-31
> **Version**: v0.557.0
> **Stage**: 30.22 (graph docs gap closure)

## Module Overview

The parser converts the flat `Vec<Token>` stream produced by the lexer
into a structured AST (`crate::ast::Crate`). Per 02-grammar.md §2-3 it
is a hand-written recursive descent + Pratt parser, with the Pratt
portion reserved for expression precedence and the recursive descent
portion handling items, types, paths, patterns, and statements.

Per Stage 6.12 (TD-022) the parser is split into 7 sibling sub-modules
(`path`, `generics`, `ty`, `expr`, `pat`, `stmt`, `items`) plus the
`parser.rs` core that retains the `Parser` struct, cursor methods, and
`parse_crate` entry point. Stage 18.03 added a `macro_expand` engine for
`macro_rules!` definitions and Stage 18.135 split out `builtin_macros`.
Two free-function entry points exist: `parse_crate(tokens, interner)`
(convenience) and `Parser::new(...).parse_crate()` (stateful).

## Data Flow Diagram

```
Vec<Token>  (from lexer::tokenize)
    │
    ▼
┌─────────────────────────────────────────────────────────────┐
│  Parser::new(tokens, &mut Rodeo)      src/parser/parser.rs    │
│                                                               │
│  tokens: Vec<Token>                                          │
│  pos: usize (cursor)                                         │
│  interner: &mut Rodeo                                        │
│  errors: Vec<ParseError>                                     │
│  no_struct_literal: bool (suppress `{` as struct lit)        │
│  shr_split / shl_split: u32 (`>>`/`<<` splitting)            │
│  last_qself: Option<QSelf> (qualified-path handoff)         │
└─────────────┬────────────────────────────────────────────────┘
              │ parse_crate()
              ▼
┌─────────────────────────────────────────────────────────────┐
│  Top-down recursive descent (src/parser/items.rs)            │
│                                                               │
│  loop: parse_visibility()? → parse_item_kind()?              │
│    - KwFn       → parse_fn() (generics, params, body)        │
│    - KwStruct   → parse_struct()                             │
│    - KwEnum     → parse_enum()                               │
│    - KwTrait    → parse_trait()                              │
│    - KwImpl     → parse_impl()                               │
│    - KwMod      → parse_mod()                                │
│    - KwUse      → parse_use_tree()                           │
│    - KwConst    → parse_const()                              │
│    - KwExtern   → parse_extern_block()                       │
│    - macro_rules! → builtin_macros::parse_macro_rules()      │
│    - macro invocation → macro_expand::expand_macro()         │
│                                                               │
│  Each sub-module adds methods to impl Parser:                 │
│    path.rs     → parse_path_with_ctx(PathContext)            │
│    generics.rs → parse_generics, parse_where_clause           │
│    ty.rs       → parse_ty (QSelf, generics, tuples, refs)    │
│    expr.rs     → parse_expr (Pratt, postfix, calls)          │
│    pat.rs      → parse_pat (struct/tuple/wild/ident)          │
│    stmt.rs     → parse_block, parse_let_stmt                 │
└─────────────┬────────────────────────────────────────────────┘
              │ Crate (AST root)
              ▼
┌─────────────────────────────────────────────────────────────┐
│  Macro expansion (src/parser/macro_expand/)                  │
│                                                               │
│  collection.rs → collect macro_rules! definitions            │
│  expansion.rs  → match macro call → substitute token trees  │
│  After expansion, re-parse substituted tokens in place        │
└─────────────┬────────────────────────────────────────────────┘
              │ (Crate, Vec<ParseError>)
              ▼
              → hir::lower::lower_crate(ast, interner)
```

## Key Data Structures

- **`Parser<'a>`** (`src/parser/parser.rs`) — Holds tokens, cursor
  (`pos`), mutable interner reference, error sink, plus GAT-aware
  `shr_split` / `shl_split` counters (Stage 18.53/18.55) and the
  `last_qself` single-use handoff field for qualified paths.
- **`PathContext`** (`src/parser/parser.rs`) — `Type | Expr | Pattern`.
  Determines whether bare `<...>` generic args are accepted (Type /
  Pattern) or whether turbofish `::<...>` is required (Expr). Closes
  the `a < b` vs `a::<b>` ambiguity from Stage 1.1 A3.
- **`Crate`** (`src/ast/mod.rs`) — AST root: `{ items: Vec<Item> }`.
  Consumed by `hir::lower::lower_crate`.
- **`ParseError` / `ParseErrorKind`** (`src/parser/error.rs`) —
  Structured error with `span` + `message` + kind; collected into
  `CompileErrors.parse` (fatal).
- **`MacroError`** (`src/parser/macro_expand/mod.rs`) — Captures
  malformed `macro_rules!`, no-matching-rule calls, recursion-limit
  violations (Stage 18.08).

## Dependencies

**Upstream inputs:**
- `Vec<Token>` from `lexer::tokenize`.
- `&mut Rodeo` (interner) for interning tuple-field indices and
  macro-generated symbols.

**Downstream consumers:**
- `src/hir/lower/mod.rs::lower_crate` — converts AST to HIR.
- `src/driver/mod.rs` — drives `parse_crate`, treats `ParseError` as
  fatal (skips HIR lowering if any present).
- `src/parser/builtin_macros/` — built-in macros (`println!`, `vec!`,
  `format!`, `compile_time_macros`) invoked during expansion.

## Stage Boundaries

Per §16, the parser consumes only `Vec<Token>` and `&mut Rodeo`; it
never reads back from HIR/MIR/typeck. Errors are fatal: the driver's
`has_fatal()` short-circuits the pipeline when `parse` errors are
non-empty. The parser sits at pipeline position 2 (after lexer, before
HIR lower). The 7-way file split (Stage 6.12 TD-022) follows §14.4
(refactoring as architecture design): each sub-module owns a grammar
section from 02-grammar.md §3.1-§3.7. The macro_expand sub-tree
(Stage 18.03) adds a macro-aware front-end that interleaves with
`items.rs` so expanded macros are re-parsed before HIR lowering.
