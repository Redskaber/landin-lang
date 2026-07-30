# Stage 14.100 — Bug AA5 Codegen Filter for Zero-Impl Default Bodies

> **Author**: redskaber
> **Date**: 2026-07-30
> **Stage**: 14.100
> **Version**: v0.113.0 → v0.114.0

## Overview

Stage 14.100 fixes an LLVM codegen crash (Bug AA5) that occurred when a trait
had default body methods but zero impls. The default body's `self.<method>()`
calls have no resolution with zero impls, causing LLVM to crash with
`Function arguments must have first-class types!`.

## LLVM Codegen Issue

### Symptom

```landin
trait Shape {
    fn area(&self) -> i32;
    fn desc(&self) -> i32 { self.area() * 100 }
}

fn main() {
    println!("hello");
}
```

Crashed with:
```
Function arguments must have first-class types!
void %"%arg0"
error: object file generation failed: LLVM module verification failed
```

### Root Cause

After Stage 14.97's fix (trait default body methods get their own DefId),
the default body `desc` is stored as a `HirItem::Fn` owner. The driver
iterates all bodies and lowers each one. When the trait has zero impls:

1. The default body's `self.area()` call has no resolution (no impl provides
   `area` for any concrete type).
2. MIR lowering falls back to `Const{ty:Error, val:Int(0)}` for the function
   pointer.
3. Codegen emits `call i32 0(...)` — a null function pointer.
4. Additionally, the function signature is wrong because `resolve_self_param_type`
   can't find an impl to specialize `Self` — it returns `None`, leaving the
   self param as `Infer` → codegen emits `void` for the param type.
5. LLVM rejects `void %(void %arg0)` — "Function arguments must have first-class types".

### Fix

Two-part fix in `src/driver.rs`:

1. **Skip codegen for zero-impl default bodies**: Before lowering each body,
   check if it's a trait default body method AND the trait has zero impls.
   If so, skip lowering entirely (dead code — can't be called without an impl).

2. **Filter `body_metas` for skipped bodies**: Track which bodies were
   actually lowered in a `HashSet<DefId>`. When building `body_metas`,
   use `filter_map` to only include lowered bodies. Without this filter,
   codegen would try to emit functions for skipped bodies, producing
   invalid LLVM IR.

### LLVM IR Pattern

**Before** (crash):
```llvm
define void @landin_Shape_default_desc(void %arg0) {  ; void param type!
  %loc_1 = alloca i32
  store void %arg0, %loc_1  ; invalid — can't store void
  ...
  %v3 = call i32 0(i32 %v2)  ; null function pointer
  ...
}
```

**After** (no function emitted — body skipped):
```llvm
; (no landin_Shape_default_desc function — body was skipped)
define i32 @landin_main() {
  ...
}
```

## Verification

- Trait with default body, zero impls → compiles and runs (no LLVM crash) ✅
- Trait with default body, one impl → still works correctly (body is lowered) ✅
- Trait with default body, 2+ impls (unoverridden) → emits Z7 error ✅
- Trait with default body, 2+ impls (all override) → no error, correct output ✅

## Architectural Notes

The fix follows §1.0 原則 1 "长期 > 短期": the fix is at the driver level
(body filtering), not a hack at codegen. The driver is the orchestrator
that knows which bodies should be lowered — codegen is a pure MIR consumer.

Per §1.0 原則 5 "报错 > 静默": silently crashing is worse than skipping
dead code. If the user actually calls the default body, they'd get a compile
error elsewhere (no impl exists to dispatch to). If they don't call it,
skipping is correct — dead code elimination.

Per §1.0 原則 6 "通用 > 特例": one rule (skip zero-impl default bodies)
handles all cases — no per-trait special-casing.
