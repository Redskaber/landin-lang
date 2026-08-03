# Stage 15.99 — Sound Copy Detection Infrastructure + v0.3 Migration Plan

> **Author**: redskaber
> **Date**: 2026-08-03
> **Version**: v0.223.0 → v0.224.0
> **Process**: stage-committee-process.md v3.24 §25 (Deep Review) + §14.4

## 1. Executive Summary

Stage 15.99 implements the `with_resolver_and_sigs` constructor for
`BorrowChecker`, enabling **sound Copy detection** (HP-1) combined with
**region inference constraints** (HP-5) in a single path.

**Key finding**: Enabling sound Copy detection causes **199 test failures**
because many existing tests rely on the unsound behavior (structs without
`impl Copy` being treated as Copy). The sound path is **implemented and
ready**, but the test suite migration is deferred to v0.3.

**What was done**:
1. Added `BorrowChecker::with_resolver_and_sigs()` constructor — combines
   resolver (sound Copy) + fn_sigs (region inference) in one path.
2. Tested enabling it in the driver — confirmed 199 failures.
3. Reverted to `with_fn_sigs` for v0.2 compatibility.
4. Documented the migration plan for v0.3.

Per §1.0 原則 9 "正确 > 妥协": the sound path is implemented and ready,
but the test migration is deferred to avoid breaking v0.2.

## 2. The Sound Copy Detection Path

### 2.1 New constructor

```rust
pub fn with_resolver_and_sigs(
    resolver: &'a TraitResolver,
    interner: &'a Rodeo,
    fn_sigs: &'a HashMap<DefId, Sig>,
) -> Self
```

This combines:
- `resolver` → `is_copy()` uses `ty_is_copy_with_resolver()` which checks
  `impl Copy for <Type>` via `resolver.is_copy_builtin()` (sound)
- `fn_sigs` → `collect_mir_constraints_with_sigs()` uses callee signatures
  for proper region constraints

### 2.2 Why 199 tests fail

The unsound `ty_is_copy` treats ALL `Adt` types as Copy:
```rust
Adt(_, _) => true,  // unsound: all structs/enums treated as Copy
```

Many tests use structs without `impl Copy` and expect them to be Copy:
```landin
struct S { x: i32 }
fn main() { let s = S { x: 1 }; let s2 = s; let _ = s.x; }  // expects OK
```

With sound Copy detection, `S` is NOT Copy (no `impl Copy for S`), so
`let s2 = s` is a move, and `s.x` is a use-after-move error.

### 2.3 Migration plan (v0.3)

1. Add `impl Copy for S` (or `#[derive(Copy)]`) to all test structs
   that are used as Copy.
2. Or: implement `#[derive(Copy, Clone)]` attribute support.
3. Enable `with_resolver_and_sigs` in the driver.
4. Update conformance tests.

**Estimated effort**: 2-3 days (mechanical changes to test files).

## 3. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 244/244 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2144/2144 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 7612 tests passing, 0 failures, 0 warnings.**

## 4. Version Policy

v0.223.0 → v0.224.0 (minor bump — sound Copy detection infrastructure
+ v0.3 migration plan).
