# Stage 15.99 — Test Plan: Sound Copy Detection Infrastructure

> **Author**: redskaber
> **Date**: 2026-08-03
> **Version**: v0.223.0 → v0.224.0

## 1. Test Scope

Stage 15.99 adds `with_resolver_and_sigs` constructor for sound Copy
detection. The sound path was tested and causes 199 failures (expected —
tests need `impl Copy` migration). The driver uses `with_fn_sigs` for
v0.2 compatibility.

## 2. Test Results

| Path | Result | Notes |
|------|--------|-------|
| `with_fn_sigs` (v0.2) | ✅ 7612/7612 PASS | Current driver path |
| `with_resolver_and_sigs` (v0.3) | 199 failures | Expected — tests need `impl Copy` |

## 3. Acceptance Criteria

| Criterion | Target | Actual |
|-----------|--------|--------|
| `cargo build` | 0 warnings | ✅ |
| `cargo fmt` | clean | ✅ |
| `cargo clippy` | 0 warnings | ✅ |
| `cargo test --lib` | 244/244 | ✅ |
| `cargo test --test all_tests` | 2144/2144 | ✅ |
| conformance | 5224/5224 | ✅ |

**Stage 15.99 PASSED**.
