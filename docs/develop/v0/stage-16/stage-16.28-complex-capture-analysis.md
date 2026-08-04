# Stage 16.28 — Closure Switch: Complex Capture Analysis + Typeck Gap

> **Author**: redskaber
> **Date**: 2026-08-03
> **Version**: v0.229.5 → v0.229.6
> **Process**: stage-committee-process.md v3.24 §1.0 原則 9 "正确 > 妥协" + 通解 > 特解

## 1. Executive Summary

Stage 16.28 applied the "通解" (general solution) approach to closure
captures: use `has_complex_captures` check to route closures with Adt
or Closure captures to inline path, while no-capture and i32-capture
closures use synthesized `call` function.

**Root cause found for complex capture failures**: synthesized closure
MIR bodies don't run typeck, so their return type stays `Infer`. This
causes type errors for closures that return non-primitive types (e.g.,
nested closures, struct captures).

**No behavior change** — all 7717 tests pass. No-capture and i32-capture
closures still use synthesized `call` function.

## 2. The "通解" Approach

Instead of fixing each capture type individually (特解), use a general
check: if the closure captures Adt or Closure types, use inline path.
Otherwise (no captures or i32 captures), use synthesized path.

```rust
let has_complex_captures = cx.closure_bodies.get(&func_local)
    .map(|info| {
        info.captures.iter().any(|(_, ty)| {
            matches!(ty.kind, TyKind::Adt(_, _) | TyKind::Closure(_, _))
        })
    }).unwrap_or(false);
```

## 3. Root Cause: Missing Typeck on Synthesized MIR

**Problem**: `f()()` where `f = || || x` — the outer closure `f` returns
a closure (inner `|| x`). The synthesized function's return type is
`Infer(TyVar)`, which never gets resolved because typeck doesn't run
on synthesized MIR bodies.

**Error**: `expected function, found ()` — the return type defaults to
`()` (unit) instead of the closure type.

**Why inline path works**: The inline path lowers the closure body
directly in the caller's MIR, so typeck resolves the return type from
the caller's context.

**Fix needed**: Run typeck on synthesized closure MIR bodies, or
infer the return type from the closure body expression's type.

## 4. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 244/244 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2249/2249 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 7717 tests passing, 0 failures, 0 warnings.**
- **Runtime**: `f(10) = 11` ✅, `x + y = 15` ✅ (i32 captures)

## 5. Version Policy

v0.229.5 → v0.229.6 (patch bump — complex capture analysis, no behavior change.)
