# Stage 16.27 — Capture Closure: fn_sigs_map Fix + Partial Success

> **Author**: redskaber
> **Date**: 2026-08-03
> **Version**: v0.229.4 → v0.229.5
> **Process**: stage-committee-process.md v3.24 §1.0 原則 9 "正确 > 妥协"

## 1. Executive Summary

Stage 16.27 fixed the root cause of the capture closure segfault:
`build_fn_sigs_map` used `mir_type_to_emit_type` which returns `Struct`
for Closure types, but the function definition expects `OpaquePtr`.
Fixed by returning `OpaquePtr` for Closure-typed params in `fn_sigs_map`.

**Partial success**: Simple i32 captures work (`x + y = 15` ✅), but
struct captures and nested closures still segfault. Reverted to inline
path for capture closures.

**No behavior change** — all 7717 tests pass. No-capture closures still
use synthesized `call` function.

## 2. The Fix

### 2.1 build_fn_sigs_map (codegen/mod.rs)

```rust
// Before: mir_type_to_emit_type returns Struct for Closure
let param_tys: Vec<EmitType> = sig.inputs.iter().map(mir_type_to_emit_type).collect();

// After: OpaquePtr for Closure-typed params
let param_tys: Vec<EmitType> = sig.inputs.iter().map(|t| {
    if matches!(t.kind, TyKind::Closure(_, _)) { EmitType::OpaquePtr }
    else { mir_type_to_emit_type(t) }
}).collect();
```

### 2.2 detect_operand_type (mir_translation.rs)

```rust
// For Closure-typed Move operands (self arg in closure calls):
if matches!(ld.ty.kind, TyKind::Closure(_, _)) && matches!(op, Operand::Move(_)) {
    EmitType::OpaquePtr
}
```

## 3. Results

| Test | Result |
|------|--------|
| `f(10)` (no-capture) | ✅ Returns 11 |
| `x + y = 15` (i32 capture) | ✅ Returns 15 |
| Struct capture (`|| p.x`) | ❌ Segfault |
| Nested closure call | ❌ Segfault |
| Capture chain | ❌ Segfault |

## 4. Remaining Issue

Struct captures and nested closures still segfault. The issue is likely
in how the closure struct's capture fields are accessed via GEP from
the self pointer when the capture type is a struct (not a primitive).

## 5. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 244/244 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2249/2249 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 7717 tests passing, 0 failures, 0 warnings.**
- **Runtime**: `f(10) = 11` ✅, `x + y = 15` ✅ (i32 captures)

## 6. Version Policy

v0.229.4 → v0.229.5 (patch bump — fn_sigs_map fix, no behavior change.)
