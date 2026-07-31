# Stage 15.16 — Error System: Spanned Trait + ErrorCode Catalog

> **Author**: redskaber
> **Date**: 2026-07-31
> **Version**: v0.141.0 → v0.142.0
> **Process**: stage-committee-process.md v3.23 §29 (stage-end deep review)

## 1. Executive Summary

Stage 15.16 adds two improvements to the error system:

1. **`Spanned` trait** — uniform span access for all error types. All 6 error
   types (LexError, ParseError, ResolveError, TypeError, BorrowError, LowerError)
   now implement `Spanned`, so consumers can access `error.span()` without
   knowing the concrete type.

2. **`ErrorCode` catalog** — stable error codes (E001-E900) for each error
   category. The `to_diagnostics` method now uses `ErrorCode::Lex.to_string()`
   instead of string literals "Lex", "Parse", etc. This makes error codes
   stable and documented.

## 2. Changes Made

### 2.1 `Spanned` trait (`src/diagnostics/mod.rs`)

```rust
pub trait Spanned {
    fn span(&self) -> Span;
}
```

Implemented for: `LexError`, `ParseError`, `ResolveError`, `TypeError`,
`BorrowError`, `LowerError`.

### 2.2 `ErrorCode` catalog (`src/diagnostics/mod.rs`)

```rust
pub enum ErrorCode {
    Lex,      // E001
    Parse,    // E100
    Lower,    // E200
    Resolve,  // E300
    Type,     // E400
    Borrow,   // E500
    Trait,    // E600
    Internal, // E900
}
```

Methods: `code()` → "E001", `category()` → "lex", `Display` impl.

### 2.3 Updated `to_diagnostics` (`src/driver.rs`)

Changed from `with_code("Lex")` (string literal) to
`with_code(ErrorCode::Lex.to_string())` (stable code).

### 2.4 8 new unit tests

Tests in `src/diagnostics/mod.rs`:
1. `stage15_16_error_code_codes` — code() returns correct codes
2. `stage15_16_error_code_categories` — category() returns correct names
3. `stage15_16_error_code_display` — Display impl
4. `stage15_16_spanned_trait_lex_error` — LexError implements Spanned
5. `stage15_16_spanned_trait_type_error` — TypeError implements Spanned
6. `stage15_16_spanned_trait_resolve_error` — ResolveError implements Spanned
7. `stage15_16_spanned_trait_borrow_error` — BorrowError implements Spanned
8. `stage15_16_spanned_trait_parse_error` — ParseError implements Spanned

### 2.5 Updated test assertions

`tests/v0/stage15/plan/driver_diagnostics_integration_tests.rs` — updated
from `Some("Lex")` to `Some("E001")` etc.

## 3. Test Results

| Test suite | Before (v0.141.0) | After (v0.142.0) | Delta |
|------------|-------------------|------------------|-------|
| Rust unit (lib) | 153 | 161 | +8 (Stage 15.16 tests) |
| Rust integration | 2006 | 2006 | 0 |
| Conformance | 5216 | 5216 | 0 |
| **Total** | **7375** | **7383** | **+8** |

All tests pass. Zero regressions. Zero clippy warnings. fmt clean.
