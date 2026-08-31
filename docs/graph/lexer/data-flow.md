# Lexer Data Flow (source → tokens)

> **Date**: 2026-08-31
> **Version**: v0.557.0
> **Stage**: 30.22 (graph docs gap closure)

## Module Overview

The lexer is the first compilation pass. It converts raw source text into a
flat token stream by walking characters with a hand-written recursive scanner
(no flex / re2c). Per 02-grammar.md §1 the lexical structure is split into 6
categories — identifiers, numbers, strings, operators, keywords, and raw
strings — each implemented as a sibling sub-module that adds methods to
`impl Lexer`. The lexer also owns symbol interning (via `lasso::Rodeo`) so
that downstream passes can compare identifiers as `Spur` (u32) instead of
strings.

The public entry point is the free function `tokenize(src, interner)`,
which returns `(Vec<Token>, Vec<LexError>)`. It mirrors the entry-point
style of sibling stages (`parse_crate`, `lower_crate`, `resolve_crate`,
`codegen_crate` — see `src/lexer/mod.rs`).

## Data Flow Diagram

```
source text (&str)
    │
    ▼
┌─────────────────────────────────────────────────────────────┐
│  Lexer::new(src, &mut Rodeo)            src/lexer/reader.rs │
│                                                               │
│  cursor: char indices                                         │
│  interner: &mut Rodeo (Symbol interner)                       │
│  errors: Vec<LexError>                                        │
└─────────────┬────────────────────────────────────────────────┘
              │ loop { next_token() }
              ▼
┌─────────────────────────────────────────────────────────────┐
│  next_token() dispatches by leading char:                    │
│                                                               │
│  whitespace / line-comment  → skip & continue                  │
│  letter / _                 → ident::read_ident()              │
│      └─ keyword_from_str()  → KwXxx / Ident(Symbol)           │
│  digit                      → number::read_number()             │
│      └─ IntLit(u128, Option<IntTy>) / FloatLit(f64, ..)      │
│  "                          → string::read_string()            │
│      └─ StrLit(Symbol) / RawStrLit(Symbol, usize)             │
│  '                          → char or Lifetime(Symbol)         │
│  operator prefix            → operators::read_operator()        │
│      └─ Plus / EqEq / Shr / ShlEq / ...                       │
│  EOF                        → TokenKind::Eof                   │
│  else                       → push LexError + recover           │
└─────────────┬────────────────────────────────────────────────┘
              │ Vec<Token>
              ▼
┌─────────────────────────────────────────────────────────────┐
│  tokenize() post-processing (src/lexer/mod.rs)              │
│                                                               │
│  - Drop error-recovery Eofs (continue lexing)                │
│  - Append real Eof once is_at_end()                          │
│  - Guarantee last token is TokenKind::Eof                    │
└─────────────┬────────────────────────────────────────────────┘
              │ (Vec<Token>, Vec<LexError>)
              ▼
              → parser::Parser::new(tokens, interner)
```

## Key Data Structures

- **`Token`** (`src/lexer/token.rs`) — `{ kind: TokenKind, span: Span }`.
  The atomic unit consumed by the parser. `span` enables accurate
  diagnostic locations downstream.
- **`TokenKind`** (`src/lexer/token.rs`) — Enum of all lexical categories:
  literals (`IntLit`, `FloatLit`, `StrLit`, `RawStrLit`, `CharLit`,
  `ByteLit`), identifiers (`Ident`, `RawIdent`, `Lifetime`), reserved
  keywords (`KwFn`, `KwStruct`, `KwImpl`, `KwSelf_`, …), operators
  (`Plus`, `EqEq`, `Shr`, `Shl`, `RArrow`, `FatArrow`, …), and `Eof`.
- **`Lexer`** (`src/lexer/reader.rs`) — Cursor struct holding
  `src: &str`, `pos: usize`, `interner: &mut Rodeo`, `errors: Vec<LexError>`.
  Each sub-module (`ident`, `number`, `string`, `operators`) extends it
  with `read_*` methods.
- **`Symbol`** — Type alias for `lasso::Spur` (u32). All identifiers,
  strings, and lifetimes are interned; downstream comparisons are integer
  equality.
- **`LexError` / `LexErrorKind`** (`src/lexer/reader.rs`) — Structured
  error type with `span` + `kind` for unterminated strings, invalid
  numeric suffixes, unknown chars, etc.

## Dependencies

**Upstream inputs:**
- Source text (`&str`) — provided by the driver or interactive caller.
- `lasso::Rodeo` — the string interner; borrowed mutably so the lexer
  can intern tuple-field indices and raw identifiers.

**Downstream consumers:**
- `src/parser/parser.rs` — consumes the `Vec<Token>` to build the AST.
- `src/driver/mod.rs` — orchestrates `tokenize`, collects `LexError`
  into `CompileErrors.lex` (fatal — aborts pipeline if non-empty).
- `src/bin/landinc.rs` — CLI entry; uses `is_valid_ident` (Stage 18.155)
  for project-name validation.

## Stage Boundaries

Per §16 (interface isolation), the lexer has no upstream IR dependency
and produces a single output artifact: the `Vec<Token>` plus a `Vec<LexError>`.
It owns the symbol interner for the entire compilation, but never reads
back from later passes. The driver treats `LexError` as fatal: if the
token stream is malformed, `parse_crate` is skipped and the driver
returns immediately with `CompileErrors.has_fatal() == true`. The lexer
sits at pipeline position 1, ahead of parser (2), hir lowering (3),
resolve (4), mir lower (5), typeck (6), drop elaboration (6.5),
borrowck (7), and codegen (8).
