# Stage 16.45 — Project-Wide Dead Code Audit

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.234.2 → v0.235.0
> **Process**: stage-committee-process.md v3.24 §1.0 原則 5 "去除兼容思维"

## 1. Executive Summary

Stage 16.45 performs a project-wide dead code audit across ALL modules (not
just codegen). It finds and removes dead code and unnecessary `#[allow]`
annotations in modules outside codegen.

**What was removed**:
1. `make_path` function in `parser/path.rs` — truly dead (never called, had `#[allow(dead_code)]`)
2. `#[allow(dead_code)]` on `Color::Bold` in `diagnostics/mod.rs` — unnecessary (the variant IS used in the `code()` match arm)

**What was kept (justified)**:
1. `async_marker` module in `ast/mod.rs` — future async/await support (Stage 8.5), documented
2. `region_inference` module in `borrowck/mod.rs` — partially used (RegionInferenceContext, RegionInferenceError are called), documented

**Test results**: 7876 tests passing, 0 failures, 0 warnings. No behavior change.

## 2. Audit Findings

### 2.1 Removed: `make_path` (parser/path.rs)

```rust
#[allow(dead_code)]
pub(super) fn make_path(...) -> Path { ... }
```

**Status**: Truly dead — never called anywhere in `src/` or `tests/`. The `Path` struct is constructed inline wherever needed. Removed the function and updated the module doc comment.

### 2.2 Removed: `#[allow(dead_code)]` on `Color::Bold` (diagnostics/mod.rs)

```rust
#[allow(dead_code)]
Bold,
```

**Status**: Unnecessary — `Color::Bold` IS used in the `code()` match arm (`Color::Bold => "\x1b"`). The `#[allow(dead_code)]` was incorrect. The variant itself is kept; only the annotation is removed.

### 2.3 Kept: `async_marker` module (ast/mod.rs)

```rust
#[allow(dead_code)]
mod async_marker;
```

**Status**: Justified — this is future async/await support (Stage 8.5). The module contains tests but no production callers yet. The `#[allow(dead_code)]` is correct because the module IS dead code until async/await is implemented. Kept as-is.

### 2.4 Kept: `region_inference` module (borrowck/mod.rs)

```rust
#[allow(dead_code)]
mod region_inference;
```

**Status**: Justified — the module IS used (`RegionInferenceContext`, `RegionInferenceError` are called from `borrowck/mod.rs`), but many internal functions are not yet called (full NLL integration is partial). The `#[allow(dead_code)]` suppresses warnings for the unused internal functions. Kept as-is.

## 3. Post-Audit #[allow] Summary

| Location | Annotation | Status |
|----------|-----------|--------|
| `ast/mod.rs:8` | `#[allow(dead_code)] mod async_marker` | ✅ Kept (future feature) |
| `borrowck/mod.rs:41` | `#[allow(dead_code)] mod region_inference` | ✅ Kept (partial integration) |
| `parser/path.rs:30` | `#[allow(dead_code)] fn make_path` | ✅ Removed (truly dead) |
| `diagnostics/mod.rs:129` | `#[allow(dead_code)] Bold` | ✅ Removed (unnecessary) |
| `codegen/*` | All removed in Stages 16.35-16.42 | ✅ Zero remaining |

**Project-wide `#[allow(dead_code)]` count**: 2 (both justified, both documented)

## 4. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 244/244 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2408/2408 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 7876 tests passing, 0 failures, 0 warnings.**

## 5. Version Policy

v0.234.2 → v0.235.0 (minor bump — removed dead function `make_path` from
parser module. API surface change: `make_path` was `pub(super)` so not
externally visible, but the removal is a structural change.)
