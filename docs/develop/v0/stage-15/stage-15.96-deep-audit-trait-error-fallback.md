# Stage 15.96 — Deep Audit: Trait Error Debug Fallback Fix

> **Author**: redskaber
> **Date**: 2026-08-03
> **Version**: v0.220.0 → v0.221.0
> **Process**: stage-committee-process.md v3.24 §25 (Deep Review) + §29 (Inter-stage Deep Verification)

## 1. Executive Summary

Stage 15.96 is a **deep audit and correction** stage per the user's
directive: "审查项目简化（设计、实现、测试），判断当前阶段是否能闭合完整".

The audit found **2 remaining `{:?}` Debug format fallback sites** in
`driver.rs` for trait errors — these were missed by Stages 15.80-15.89
because they only triggered when the interner was `None` (test contexts).

**Fix**: Added `TraitError::format_without_interner()` method that
produces human-readable messages without an interner, using `<unknown>`
for unresolved symbols instead of Debug format.

**Before** (no interner, e.g., test context):
```
[trait] Coherence(CoherenceError { trait_name: Spur(5), self_ty_name: Spur(7), impl_def_ids: [DefId(1), DefId(2)], span: Span { lo: 21, hi: 37 } })
```

**After**:
```
[trait] conflicting implementations of trait `<unknown>` for type `<unknown>` (2 impl blocks)
```

**Test impact**: 0 new tests (no behavior change for normal compilation
with interner; only affects fallback path).
- **Total: 7612 tests passing**, 0 failures, 0 warnings.

Per §1.0 原則 4 "报错 > 静默": errors are always human-readable, even
without an interner.

## 2. Audit Findings

### 2.1 Remaining `{:?}` Debug fallbacks (2 sites)

| Location | Code | Issue |
|----------|------|-------|
| `driver.rs:279` (format_for_user) | `format!("{:?}", e)` | Trait error Debug fallback when interner is None |
| `driver.rs:364` (to_diagnostics) | `format!("{:?}", e)` | Same fallback in diagnostic path |

Both sites were in the `else` branch of `if let Some(interner) = interner`,
which is the fallback path used in test contexts (where `format_for_user`
is called without an interner).

### 2.2 Other audit findings (no action needed)

| Item | Status | Notes |
|------|--------|-------|
| `borrowck/mod.rs:218` Span::DUMMY TODO | Deferred | Region inference lifetime error span — requires constraint cause tracking (deep refactor) |
| `mir/lower/mod.rs` panic! in tests | OK | Test-only code, expected |
| `lexer/reader.rs:332` {:?} in error message | OK | `unexpected character: {:?}` — char Debug is acceptable (no human-readable alternative for arbitrary chars) |
| `codegen/llvm/mod.rs:421` {:?} for cache key | OK | Internal cache key, not user-facing |
| `resolve/resolver.rs:114,123` {:?} for Debug impl | OK | Debug impl, not user-facing error |

## 3. The Fix

### 3.1 New `format_without_interner` method

```rust
pub fn format_without_interner(&self) -> String {
    match self {
        TraitError::Coherence(ce) => {
            format!(
                "conflicting implementations of trait `<unknown>` for type `<unknown>` ({} impl blocks)",
                ce.impl_def_ids.len()
            )
        }
        TraitError::Incomplete(inc) => {
            format!(
                "impl `<unknown>` for `<unknown>` is missing method(s): <{} method(s)>",
                inc.missing_methods.len()
            )
        }
    }
}
```

### 3.2 Updated fallback paths

Both `format_for_user` and `to_diagnostics` now call
`e.format_without_interner()` instead of `format!("{:?}", e)`.

## 4. API Naming Compliance (§23)

**New public method**:

| Method | Location | §23 Compliance |
|--------|----------|-----------------|
| `format_without_interner(&self) -> String` | `driver::TraitError` | ✅ `<verb>_<prep>_<noun>` pattern |

## 5. Verification

- `cargo clean && cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 244/244 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2144/2144 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 7612 tests passing, 0 failures, 0 warnings.**

## 6. Version Policy

v0.220.0 → v0.221.0 (minor bump — Phase 2 deep audit correction).
