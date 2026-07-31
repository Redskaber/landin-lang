# Stage 15.16 — Test Plan

> **Date**: 2026-07-31
> **Version**: v0.141.0 → v0.142.0

## 1. Test Scope

| Area | Test type | Count |
|------|-----------|-------|
| ErrorCode catalog | Unit (lib) | 3 new |
| Spanned trait | Unit (lib) | 5 new |
| Regression | All existing | 2006 + 5216 |

## 2. Test cases

| # | Test name | Verifies |
|---|-----------|----------|
| 1 | `stage15_16_error_code_codes` | code() returns E001/E100/.../E900 |
| 2 | `stage15_16_error_code_categories` | category() returns lex/parse/... |
| 3 | `stage15_16_error_code_display` | Display impl produces "E001" |
| 4 | `stage15_16_spanned_trait_lex_error` | LexError.span() |
| 5 | `stage15_16_spanned_trait_type_error` | TypeError.span() |
| 6 | `stage15_16_spanned_trait_resolve_error` | ResolveError.span() |
| 7 | `stage15_16_spanned_trait_borrow_error` | BorrowError.span() |
| 8 | `stage15_16_spanned_trait_parse_error` | ParseError.span() |
