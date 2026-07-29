# Stage 14.82 — Closure Struct Capture Fix

> **Date**: 2026-07-30
> **Version**: v0.97.0 → v0.98.0
> **Process**: stage-committee-process.md v3.22 §11.3 (LLVM doc sync)

## Overview

This document describes the LLVM-related aspects of the Stage 14.82 fix
for closure struct captures. Per §11.3, any change to LLVM type generation
requires a doc sync to `docs/llvm/`.

## Background

Closures in Landin are represented as anonymous struct values with one
field per capture:

```rust
let p = Point { x: 10, y: 20 };
let f = || p.x;
```

The closure `f` has type `Closure(def_id, [Point])` — an anonymous struct
`{ Point }` (one field of type Point).

In LLVM IR, this becomes:

```llvm
%loc_f = alloca { { i32, i32 } }  ; closure struct: one Point field
```

## The bug

The closure struct was typed `{ i32 }` (one i32 field) instead of
`{ { i32, i32 } }` (one Point field), causing LLVM verification to fail:

```
Invalid InsertValueInst operands!
  %v9 = insertvalue { i32 } undef, { i32, i32 } %v8, 0
```

## Root cause (3 layers)

### Layer 1: `mir_type_to_emit_type` (legacy) used for closure substs

In `src/codegen/emitter.rs` line 506-508:

```rust
TyKind::Closure(_, substs) => {
    let fields: Vec<EmitType> = substs.iter().map(mir_type_to_emit_type).collect();
    EmitType::Struct(fields)
}
```

`mir_type_to_emit_type` is the legacy variant WITHOUT `AdtLayouts`. For
`Adt(Point)`, it falls back to `EmitType::I32` (the catch-all on line 515).

### Layer 2: `mir_type_to_emit_type_with_layouts` had no `Closure` arm

In `src/codegen/mir_translation.rs`, the layouts-aware variant
`mir_type_to_emit_type_with_layouts` did NOT have an explicit
`TyKind::Closure` arm — it fell through to the legacy variant (Layer 1).

### Layer 3: Closure substs held stale `Infer(TyVar)` after typeck

In `src/driver.rs`, the MIR-lower pass produces:

```rust
// At MIR-lower time, p's type is Infer(TyVar) (typeck hasn't run yet)
let closure_ty = Ty::new(
    TyKind::Closure(closure_def_id, capture_tys.clone()),  // capture_tys = [Infer(TyVar)]
    expr.span,
);
```

After typeck writes back `Adt(Point)` to `p`'s `local_decl.ty`, the
closure's substs still hold the stale `Infer(TyVar)`.

## Fix

### Fix 1: Add explicit `Closure` arm to `mir_type_to_emit_type_with_layouts`

```rust
TyKind::Closure(_, substs) => {
    let fields: Vec<EmitType> = substs
        .iter()
        .map(|ty| mir_type_to_emit_type_with_layouts(ty, layouts))
        .collect();
    EmitType::Struct(fields)
}
```

This recurses with layouts, so `Adt(Point)` captures resolve to their
actual LLVM struct type (`{ i32, i32 }`).

### Fix 2: Use layouts-aware variant in `rvalue.rs` `AggregateKind::Closure`

```rust
let field_tys: Vec<EmitType> = substs
    .iter()
    .map(|ty| mir_type_to_emit_type_with_layouts(ty, layouts))
    .collect();
```

### Fix 3: `populate_adt_layouts` recurses into `Closure` substs

In `src/mir/lower/adt_layout.rs`:

- `collect_adt_def_ids` now recurses into `TyKind::Closure(_, substs)`
- `populate_adt_layouts` walks `Aggregate(Closure, ...)` rvalues' substs

This ensures captured Adts have their layouts registered in
`mir.adt_layouts`, which is required by Fix 1.

### Fix 4: Driver writeback for closure substs

In `src/driver.rs`, after typeck, walk every
`Aggregate(Closure, operands)` rvalue. For each operand that's a
Copy/Move of a local, look up the local's resolved type and write it
back to the corresponding subst:

```rust
for bb in &mut mir.basic_blocks {
    for stmt in &mut bb.statements {
        if let StatementKind::Assign(boxed) = &mut stmt.kind {
            let (_, rv) = &mut **boxed;
            if let Rvalue::Aggregate(AggregateKind::Closure(_, substs), operands) = rv {
                let src_local_ids: Vec<_> = operands.iter().map(...).collect();
                for (i, src_id_opt) in src_local_ids.iter().enumerate() {
                    if let Some(src_id) = src_id_opt {
                        if let Some(src_ld) = mir.local_decls.get(src_id.0 as usize) {
                            let src_ty = src_ld.ty.clone();
                            if !matches!(&src_ty.kind, TyKind::Infer(_)) {
                                if i < substs.len() {
                                    substs[i] = src_ty;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
// Re-populate adt_layouts after closure subst writeback
populate_adt_layouts(&mut mir, &hir);
```

## Verification

### Before fix

```llvm
%loc_5 = alloca { i32 }              ; WRONG: should be { { i32, i32 } }
%v4 = load { i32, i32 }, %loc_4      ; loads Point struct
%v5 = insertvalue { i32 } undef, i32 %v4, 0  ; CRASH: type mismatch
```

### After fix

```llvm
%loc_5 = alloca { { i32, i32 } }     ; CORRECT: closure struct with Point field
%v4 = load { i32, i32 }, %loc_4      ; loads Point struct
%v5 = insertvalue { { i32, i32 } } undef, { i32, i32 } %v4, 0  ; OK
```

## Known limitation: disjoint captures (RFC 2229)

True disjoint field-level captures are NOT implemented:

```rust
let p = Point { x: 10, y: 20 };
let f = || p.x;  // should capture only p.x (i32), not whole p
let g = || p.y;  // should capture only p.y (i32), not whole p
```

Currently both closures capture the whole `p` struct. The simple case
works; the disjoint case produces garbage for the second closure.

Implementing RFC 2229 requires `closure_capture.rs::collect_captured_locals`
to track field-level captures (e.g., `CapturedPlace::Field(p, 0)` instead
of `CapturedPlace::Local(p)`). This is a deeper change deferred to
Stage 14.83+.

## Related docs

- `docs/develop/v0/stage-14/gate-review-14.82.md` — gate review
- `docs/develop/v0/stage-14/dev-log.md` — dev log entry
- `RELEASE_NOTES.md` — v0.98.0 entry
- `docs/worklog.md` — Stage 14.82 worklog entry
