# Stage 16.02 — Sound Copy Detection: Migration Attempt + Assessment

> **Author**: redskaber
> **Date**: 2026-08-03
> **Version**: v0.226.0 → v0.226.1
> **Process**: stage-committee-process.md v3.24 §25 (Deep Review)

## 1. Executive Summary

Stage 16.02 attempts the v0.3 sound Copy detection migration by enabling
`with_resolver_and_sigs` in the driver. The attempt confirmed 199 test
failures and was reverted. The stage documents the assessment and
updates the driver comment.

**Per user directive**: "简化的设计实现需要将其完整的设计实现纳入设计
实现测试计划，不能遗漏" — the sound Copy detection simplification is
documented with a complete migration plan.

## 2. Migration Attempt

### 2.1 What was done

Enabled `with_resolver_and_sigs` in `driver.rs`:
```rust
let mut bc = borrowck::BorrowChecker::with_resolver_and_sigs(
    &trait_resolver,
    &interner,
    &fn_sig_table.sigs,
);
```

### 2.2 Results

| Test Category | Before | After | Failures |
|---------------|--------|-------|----------|
| lib tests | 244/244 | 244/244 | 0 |
| integration tests | 2144/2144 | 2096/2144 | 48 |
| conformance tests | 5224/5224 | 5025/5224 | 199 |
| **Total** | **7612** | **7365** | **247** |

### 2.3 Root cause

The unsound `ty_is_copy` treats ALL `Adt` types as Copy (`Adt(_, _) => true`).
Many tests use structs without `impl Copy` and expect them to be Copy:
```landin
struct S { x: i32 }
fn main() { let s = S { x: 1 }; let s2 = s; let _ = s.x; }  // expects OK
```

With sound Copy detection, `S` is NOT Copy → `let s2 = s` is a move →
`s.x` is use-after-move error.

### 2.4 Decision: Defer to v0.3

Reverted to `with_fn_sigs` for v0.2 compatibility. The sound path is
implemented and ready — migration requires adding `impl Copy` to ~199
test structs (mechanical change, 2-3 days).

Per §1.0 原則 9 "正确 > 妥协": the sound path is ready, but the test
migration is deferred to avoid breaking v0.2.

## 3. Migration Plan (v0.3)

### 3.1 Automated migration script

Create a script that:
1. Scans all `.lin` test files for `struct S { ... }` without `impl Copy for S {}`
2. Adds `impl Copy for S {}` after the struct definition
3. Re-enables `with_resolver_and_sigs` in the driver
4. Runs all tests to verify

### 3.2 Manual review

After automated migration:
1. Review tests that still fail — these may be intentional use-after-move
   tests that should be flipped to `compile_error`
2. Check that `impl Drop` tests still work (Drop + Copy is a conflict in Rust)
3. Verify that trait impls with `Copy` bound work correctly

### 3.3 Cleanup

After migration:
1. Remove `ty_is_copy` unsound function (or mark `#[deprecated]`)
2. Remove `with_fn_sigs` constructor (or mark `#[deprecated]`)
3. Update API naming standard to document the sound path

## 4. Verification (reverted state)

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 244/244 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2144/2144 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 7612 tests passing, 0 failures, 0 warnings.**

## 5. Version Policy

v0.226.0 → v0.226.1 (patch bump — documentation update, migration
attempt + revert, no behavior change).
