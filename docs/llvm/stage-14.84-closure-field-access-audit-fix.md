# Stage 14.84 — Closure Field Access Audit Fix

> **Date**: 2026-07-30
> **Version**: v0.99.0 → v0.100.0
> **Process**: stage-committee-process.md v3.22 §11.3 (LLVM doc sync)

## Overview

This document describes the LLVM-related aspects of the Stage 14.84 audit
fix for closure field access. The Stage 14.82 "partial fix" for GAP-7 only
worked for field 0 access (`.x`) — accessing field 1+ (`.y`) silently
returned garbage from uninitialized memory.

## Background

Closures in Landin capture whole locals. For example:

```rust
struct Point { x: i32, y: i32 }
fn main() -> i32 {
    let p = Point { x: 10, y: 20 };
    let f = || p.x;  // captures whole p, loads .x at call site
    let g = || p.y;  // captures whole p, loads .y at call site
    println!("{}", f());  // 10
    println!("{}", g());  // 20
    0
}
```

Each closure is an anonymous struct `{ Point }` (one field of type Point).
At the call site, the inlined body extracts the capture from field 0 of
the closure struct, then loads the desired field from the extracted Point.

## The bug

The Stage 14.82 fix updated `AggregateKind::Closure` substs (used for
`insertvalue` type) but NOT:

1. The closure local's `local_decl.ty` (used for `alloca` size + `store` type)
2. The user-visible `f` local's type (assigned via `f = Move(closure_tmp)`)
3. The extract local's type (created by `lower_closure_call_inline`)

Resulting LLVM IR (broken):

```llvm
%loc_5 = alloca { i32 }              ; WRONG: should be { { i32, i32 } }
%loc_6 = alloca { i32 }              ; WRONG: should be { { i32, i32 } }
%v5 = insertvalue { { i32, i32 } } undef, { i32, i32 } %v4, 0  ; OK (AggregateKind substs updated)
store { { i32, i32 } } %v5, %loc_5   ; TYPE CONFUSION: alloca is { i32 } but store is { { i32, i32 } }
```

LLVM silently truncated the 8-byte store to the 4-byte alloca, keeping
only field 0 (`.x = 10`). Accessing `.y` (field 1) read bytes 4-8 of a
4-byte alloca → uninitialized memory → garbage output.

## Why e2e-runok-142 passed before

The original test only accessed `.x` (field 0), which is in the first 4
bytes — accidentally correct due to LLVM's silent truncation.

## Fix (4 layered)

### Fix 1: Update closure local's `local_decl.ty`

In `src/driver.rs`, after updating `AggregateKind::Closure` substs, also
rebuild the closure local's `local_decl.ty` with the resolved substs:

```rust
if let Some(lhs_id) = lhs_local_id {
    if let Some(lhs_ld) = mir.local_decls.get_mut(lhs_id.0 as usize) {
        let new_closure_ty = crate::mir::ty::Ty::new(
            crate::mir::ty::TyKind::Closure(closure_def_id, substs.clone()),
            lhs_ld.ty.span,
        );
        lhs_ld.ty = new_closure_ty;
    }
}
```

This fixes the closure tmp local's alloca size.

### Fix 2: Propagate closure types through `Move`

In `src/driver.rs`, walk `Rvalue::Use(Operand::Move(closure_tmp))`
statements and propagate the resolved `Closure` type from `closure_tmp`
to the user-visible `f` local.

### Fix 3: Update extract locals' types

In `src/driver.rs`, walk
`extract_local = Use(Copy(Projection(closure_local, Field(i, _))))`
statements and update `extract_local.ty` from the closure local's resolved
subst at field index `i`.

### Fix 4: Codegen type lookup for Closure base

In `src/codegen/mir_translation.rs::detect_place_type`, when
`ProjectionElem::Field` has Infer `field_ty` AND the base is a Closure
local, look up the field type from the closure's substs:

```rust
ProjectionElem::Field(field_id, field_ty) => {
    let emit_ty = mir_type_to_emit_type_with_layouts(field_ty, layouts);
    if matches!(emit_ty, EmitType::I32)
        && matches!(&field_ty.kind, TyKind::Infer(_))
    {
        if let PlaceKind::Local(base_id) = &base.kind {
            if let Some(base_ld) = mir.local_decls.get(base_id.0 as usize) {
                if let TyKind::Tuple(field_tys) = &base_ld.ty.kind {
                    if let Some(resolved) = field_tys.get(field_id.0 as usize) {
                        return mir_type_to_emit_type_with_layouts(resolved, layouts);
                    }
                }
                // Stage 14.84 (audit fix): Also handle Closure base
                if let TyKind::Closure(_, substs) = &base_ld.ty.kind {
                    if let Some(resolved) = substs.get(field_id.0 as usize) {
                        return mir_type_to_emit_type_with_layouts(resolved, layouts);
                    }
                }
            }
        }
    }
    emit_ty
}
```

This fixes the load type — was `load i32` (4 bytes), now `load { i32, i32 }`
(8 bytes, the Point struct).

## Verification

### Before fix

```llvm
%loc_5 = alloca { i32 }              ; WRONG
%loc_6 = alloca { i32 }              ; WRONG
%v7 = getelementptr ... { i32 }* %loc_6, i32 0, i32 0
%v8 = load i32, %v7                  ; WRONG: should be load { i32, i32 }
store { i32, i32 } %v8, %loc_7       ; TYPE CONFUSION
```

### After fix

```llvm
%loc_5 = alloca { { i32, i32 } }     ; CORRECT
%loc_6 = alloca { { i32, i32 } }     ; CORRECT
%v7 = getelementptr ... { { i32, i32 } }* %loc_6, i32 0, i32 0
%v8 = load { i32, i32 }, %v7         ; CORRECT: loads Point struct
store { i32, i32 } %v8, %loc_7       ; OK
%v9 = getelementptr ... { i32, i32 }* %loc_7, i32 0, i32 1
%v10 = load i32, %v9                 ; loads .y (field 1) = 20
```

## Test coverage

Updated `e2e-runok-142-closure-struct-capture.lin` to test BOTH `.x` and
`.y` access:

```landin
struct Point { x: i32, y: i32 }
fn main() -> i32 {
    let p = Point { x: 10, y: 20 };
    let f = || p.x;
    let g = || p.y;
    println!("{}", f());  // 10
    println!("{}", g());  // 20
    0
}
```

`EXPECTED_STDOUT: 10\n20` — verifies both fields work correctly.

## Round 2 audit verification

The Round 2 audit subagent verified the fix works for:
- Single struct field access (`.x`, `.y`) ✅
- 3-field struct access (`.a`, `.b`, `.c`) ✅
- Tuple field access (`.0`, `.1`, `.2`) ✅
- Array element access (`[0]`, `[1]`, `[2]`) ✅
- Nested struct access (`o.inner.val`) ✅
- Multiple captures in one closure ✅
- Mixed capture types (struct + tuple + array) ✅
- Two closures capturing different fields ✅

## Lessons learned

1. **Always test field 1+ access**, not just field 0. Field 0 is often
   accidentally correct due to LLVM's silent truncation.
2. **Type writeback must update ALL places the type appears** — not just
   one. The Stage 14.82 fix updated `AggregateKind::Closure` substs but
   missed `local_decl.ty`, the user-visible local, and the extract local.
3. **Independent audits are valuable** — the audit subagent found this
   bug by testing beyond the conformance suite's coverage. The conformance
   test `e2e-runok-142` only tested field 0, masking the bug.

## Related docs

- `docs/develop/v0/stage-14/gate-review-14.84.md` — gate review
- `docs/llvm/stage-14.82-closure-struct-capture.md` — Stage 14.82 (initial fix)
- `RELEASE_NOTES.md` — v0.100.0 entry
- `docs/worklog.md` — Stage 14.84 worklog entry
