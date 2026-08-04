# Error System Data Flow

> **Date**: 2026-08-04
> **Version**: v0.235.1

## Error Types by Pipeline Stage

```
┌─────────────────────────────────────────────────────────────────┐
│                        Error Type Hierarchy                      │
│                                                                  │
│  Each stage has its own error type, all sharing:                 │
│    { message: String, span: Span }                              │
│                                                                  │
│  CompileErrors (driver.rs) aggregates all:                       │
│    lex:       Vec<LexError>                                      │
│    parse:     Vec<ParseError>                                    │
│    resolve:   Vec<ResolveError>                                  │
│    typeck:    Vec<TypeError>                                     │
│    borrowck:  Vec<BorrowError>                                   │
│    trait_errors: Vec<TraitError>                                 │
└─────────────────────────────────────────────────────────────────┘
```

## Error Flow Through Pipeline

```
Source Code
    │
    ▼
┌──────────────┐     Vec<LexError>
│   Lexer       │────→ errors.lex (tokenization failures)
└──────┬───────┘     Span: byte offset range
       │
       ▼
┌──────────────┐     Vec<ParseError>
│   Parser      │────→ errors.parse (syntax violations)
└──────┬───────┘     Span: token range
       │
       ▼
┌──────────────┐     Vec<ResolveError>
│  HIR Lower    │────→ errors.resolve (unresolved names)
│  + Resolver   │     Span: HIR node span
└──────┬───────┘
       │
       ▼
┌──────────────┐     Vec<TypeError>
│  MIR Lower    │────→ errors.typeck (from lowering)
│               │     e.g., "no method found", "field not found"
└──────┬───────┘
       │
       ▼
┌──────────────┐     Vec<TypeError>
│  Type Check   │────→ errors.typeck (type mismatches)
│  (iterative   │     e.g., "expected i32, found bool"
│   for closures)│     Span: MIR statement/terminator span
└──────┬───────┘
       │
       ▼
┌──────────────┐     Vec<BorrowError>
│  Borrow Check │────→ errors.borrowck (use-after-move, etc.)
│  (NLL +       │     Span: MIR statement span
│   regions)    │     e.g., "cannot assign twice to immutable"
└──────┬───────┘
       │
       ▼
┌──────────────┐
│  Codegen      │────→ (no errors — codegen is total on valid MIR)
│  (run_codegen │     LLVM verification errors → panic (P0 bug)
│   _pipeline)  │
└──────────────┘
```

## Error Reporting

```
CompileErrors
    │
    ├─ has_errors() → bool
    │     true if any error vector is non-empty
    │
    ├─ format() → String
    │     Renders all errors with spans:
    │     error[E400]: expected function, found _
    │       --> file.lin:1:49
    │       |
    │     1 | fn main() { let f = || || x; f()(); }
    │       |                                                 ^
    │
    └─ Color support (diagnostics/mod.rs)
          Color::Red    — error messages
          Color::Yellow — warnings
          Color::Cyan   — notes
          Color::Green  — success
```

## Error Codes

| Code | Stage | Example |
|------|-------|---------|
| E1xx | Lex | E100: unexpected character |
| E2xx | Parse | E200: expected `}`, found `;` |
| E3xx | Resolve | E300: unresolved name `foo` |
| E4xx | Typeck | E400: expected function, found `_` |
| E5xx | Borrowck | E500: use of moved value |

## Design Principles

- **§1.0 原則 4 "报错 > 静默"**: All errors are reported, never silently swallowed
- **§23 rule 8**: All error types use `Error` suffix, share `{ message, span }` structure
- **§16**: Errors carry span data from the stage that detected them
