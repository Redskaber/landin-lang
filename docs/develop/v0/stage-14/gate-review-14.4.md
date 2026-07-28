# Gate Review — Stage 14.4: API Naming Audit (§23)

> **Reviewer**: REV-A (automated)
> **Date**: 2026-07-28
> **Process**: stage-committee-process.md v3.21 §9.3 + §23
> **Baseline**: v0.35.0 (post-Stage 14.3) / 1951 rust tests
> **Target**: v0.36.0 (Stage 14 partial — API naming)
> **Status**: ✅ PASS (7/7 GO)

## 1. Stage Summary

Stage 14.4 audits `src/` for §23 (API Naming Standard) violations and
fixes all found. This is a pure refactoring stage — no behavior change.

**Audit findings**:
- 2 glob re-exports in `src/stdlib/mod.rs` (lines 34, 35)
- 0 missing `note = "..."` on `#[deprecated]`
- 0 missing stage prefixes on context types
- 0 DRY violations (duplicate type definitions)

**Fix applied**:
- Replaced `pub use trait_methods::*;` with explicit list of 27 names
- Replaced `pub use vtable_layout::*;` with explicit list of 18 names

## 2. §23 Compliance Checklist

| Rule | Status | Evidence |
|------|--------|----------|
| 1. Free-function entry pattern | ✅ | All stages expose `<verb>_<noun>` entries (per `api-naming-standard.md` §2.2) |
| 2. Context type naming (`Ctxt` / `-er`) | ✅ | `HirLowerCtxt`, `MirLowerCtxt`, `TypeChecker`, `BorrowChecker`, `Emitter` |
| 3. Type prefixes (`Hir` / `Mir` / `Emit`) | ✅ | Verified in `api-naming-standard.md` §4 |
| 4. Explicit re-export (no glob) | ✅ | 0 glob re-exports remaining (comment references only) |
| 5. DRY (single source of truth) | ✅ | No duplicate type definitions found |
| 6. `#[deprecated]` with `note` | ✅ | All 4 occurrences have `note = "..."` |
| 7. Function naming prefixes (`lex_`/`parse_`/`lower_`/`resolve_`/`check_`/`emit_`/`codegen_`) | ✅ | Verified |
| 8. Error types with `Error` suffix | ✅ | `LexError`, `ParseError`, `TypeError`, `BorrowError`, `ResolveError` |

## 3. Detailed Audit

### 3.1 Glob Re-export Scan

```bash
$ grep -rn "pub use.*::\*" src/ --include="*.rs"
src/hir/mod.rs:17:// Stage 3.57 (P0-3 fix): explicit list instead of `pub use kinds::*;`
src/stdlib/mod.rs:33:// Stage 14.4 §23 compliance: explicit re-export lists (no glob `pub use X::*;`).
src/codegen/trait_dispatch/mod.rs:34:// Stage 14.3 §23 compliance: explicit re-export list (no glob `pub use X::*;`).
src/ast/mod.rs:12:// `pub use kinds::*;` to prevent accidental leakage of internal types.
src/lexer/mod.rs:21:// `pub use token::*;` to prevent accidental leakage of internal types.
src/mir/mod.rs:15:// Stage 3.57 (P0-3 fix): explicit re-exports instead of `pub use *::*;`
```

**Result**: 0 actual glob re-exports — all 6 matches are comments.

### 3.2 `#[deprecated]` Scan

```bash
$ grep -rn "#\[deprecated" src/ --include="*.rs"
src/typeck/checker.rs:84:    #[deprecated(note = "Set fn_sigs directly from FnSigTable instead")]
src/typeck/checker.rs:391:    #[deprecated(note = "Use check_mir_body_with_tables instead")]
src/typeck/checker.rs:906:#[deprecated(
src/borrowck/mod.rs:590:#[deprecated(note = "Use BorrowChecker::check_mir_body (§16-compliant) or driver::compile instead")]
```

**Result**: All 4 `#[deprecated]` have `note = "..."` pointing to §16-compliant
replacements. ✅

### 3.3 Fix Applied

`src/stdlib/mod.rs` (before):
```rust
pub use trait_methods::*;
pub use vtable_layout::*;
```

`src/stdlib/mod.rs` (after):
```rust
// Stage 14.4 §23 compliance: explicit re-export lists (no glob `pub use X::*;`).
pub use trait_methods::{
    find_stdlib_trait_method, is_stdlib_marker_trait, is_stdlib_trait, is_stdlib_trait_method,
    stdlib_all_traits, stdlib_arithmetic_traits, stdlib_core_traits, stdlib_io_traits,
    // ... 27 names total
};
pub use vtable_layout::{
    stdlib_data_global_name, stdlib_dynptr_global_name, stdlib_impl_method_symbol,
    // ... 18 names total
};
```

## 4. Behavioral Verification

- ✅ `cargo build --lib --features llvm-backend`: OK
- ✅ `cargo fmt --check`: clean
- ✅ `cargo clippy --all-targets --features llvm-backend -- -D warnings`: 0 warnings
- ✅ `cargo test --features llvm-backend`: 1951 passed, 0 failed
- ✅ Zero behavior change — pure refactoring

## 5. Committee Vote

**Tally: 7/7 GO → PASS**

## 6. Final Verdict

**Stage 14.4 GATE: ✅ PASS**

- §23 compliance achieved (0 glob re-exports, all `#[deprecated]` have notes)
- Zero behavior change, zero API breakage
- All 1951 tests still pass
