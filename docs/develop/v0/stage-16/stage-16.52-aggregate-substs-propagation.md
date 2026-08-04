# Stage 16.52 — Task 11 Phase 1c: `AggregateKind::Adt` Substs Propagation

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.237.0 → v0.238.0
> **Process**: stage-committee-process.md v3.24 §13.4 + §23

## 1. Executive Summary

Stage 16.52 completes Task 11 Phase 1c — propagating parsed generic args
from HIR path segments into `AggregateKind::Adt` at all 5 sites in
`mir/lower/expr_operand.rs`. Phase 1b (Stage 16.51) propagated substs into
`TyKind::Adt` for type annotations; Phase 1c extends this to aggregate
construction so the two stay consistent under typeck unification.

**What was implemented**:

1. **5 `AggregateKind::Adt` sites updated** in `src/mir/lower/expr_operand.rs`:
   - Site 1: Enum unit variant path (`Color::Red` for unit variant)
   - Site 2: ADT ctor call (`Pair(1, 2)` via `Call` form)
   - Site 3: Struct literal (`Pair { a: 1, b: 2 }`)
   - Site 4: Enum struct variant (`Shape::Circle { r: 1.0 }`)
   - Site 5: Fall-through ADT ctor path (`Color::Some` non-unit variant)
2. **`typeck/unify.rs` Adt unification reworked** — replaced the temporary
   Stage 16.51 relaxation with a principled rule:
   - Empty substs on either side → unify by DefId only (inference case)
   - Both non-empty → must match in length and unify element-wise
3. **15 integration tests** added in
   `tests/v0/stage16/plan/stage16_52_aggregate_substs_tests.rs` covering:
   - Generic struct literals (annotated + inferred)
   - Generic enum tuple/struct/unit variants (annotated)
   - Generic enum in return position and match scrutinee
   - MIR substs propagation verification
   - No regressions on non-generic code
   - Empty/non-empty substs unification (inference edge case)

**Key result**: `let x: Opt<i32> = Opt::Some(42);` now compiles
end-to-end. The type annotation's substs `[i32]` (in `TyKind::Adt`) and
the aggregate's substs (empty, due to inference) unify correctly via the
"empty substs = unknown" rule.

**Test results**: 7926 tests passing (250 lib + 2452 integration + 5224
conformance), 0 failures, 0 warnings. +15 new tests.

## 2. Design Decisions

### 2.1 Single Helper, Multiple Call Sites (通解 > 特解)

All 4 path-based sites use the same `lower_path_generic_args(path, &mut 0)`
helper from Stage 16.51. This is consistent with §1.0 原則 6 "通用 > 特例"
— one helper for all paths, whether they appear in type annotations or
aggregate construction.

The Call-based site (Site 2) reuses the `adt_substs` already extracted
from `func_local_decl.ty.kind` — no new helper needed. This is consistent
with §23 rule 5 (DRY).

### 2.2 Empty Substs Semantics (显式 > 隐式)

The unify.rs rule is:

| LHS substs | RHS substs | Behavior |
|------------|------------|----------|
| empty      | empty      | Match by DefId (both unknown) |
| empty      | non-empty  | Match by DefId (LHS unknown, RHS known) |
| non-empty  | empty      | Match by DefId (LHS known, RHS unknown) |
| non-empty  | non-empty  | Must match in length + unify element-wise |

This is sound because empty substs are equivalent to "no information"
(the type is generic but instantiation is unknown). The actual
instantiation will be filled in by Phase 2 (substitution) once type
inference back-propagates substs from type annotations to path
expressions.

Per §1.0 原則 3 "显式 > 隐式": substs are explicit in MIR when present,
and the absence of substs is an explicit signal of "unknown" (not an
error to silently paper over).

### 2.3 Revert of Stage 16.51 Temporary Relaxation

Stage 16.51 introduced a temporary relaxation: "if substs lengths differ,
skip substs comparison". This was needed because AggregateKind::Adt had
empty substs while TyKind::Adt had non-empty substs (Phase 1b vs Phase 1c
gap).

Stage 16.52 reverts this relaxation and replaces it with the principled
"empty substs = unknown" rule. The relaxation is no longer needed because
Phase 1c makes the substs consistent across both TyKind::Adt and
AggregateKind::Adt — when substs are present, they're now correct on
both sides.

## 3. Changes

### 3.1 `src/mir/lower/expr_operand.rs` — 5 Sites

**Import**: Added `lower_path_generic_args` to the `super::{}` import list.

**Site 1 (line ~449)** — Enum unit variant path:
```rust
// Before:
let adt_ty = Ty::new(TyKind::Adt(def_id, Vec::new().into()), expr.span);
// ...
AggregateKind::Adt(def_id, variant_idx, Vec::new().into(), field_tys)

// After:
let substs = lower_path_generic_args(path, &mut 0);
let adt_ty = Ty::new(TyKind::Adt(def_id, substs.clone()), expr.span);
// ...
AggregateKind::Adt(def_id, variant_idx, substs, field_tys)
```

**Site 2 (line ~819)** — Call-based ADT ctor:
```rust
// Before: AggregateKind::Adt(adt_def_id, variant_idx, Vec::new().into(), field_tys)
// After:  AggregateKind::Adt(adt_def_id, variant_idx, adt_substs, field_tys)
//         (adt_substs already extracted from func_local_decl.ty.kind)
```

**Site 3 (line ~1844)** — Struct literal:
```rust
// Before:
let struct_ty = Ty::new(TyKind::Adt(def_id, Vec::new().into()), expr.span);
AggregateKind::Adt(def_id, 0, Vec::new().into(), field_tys)

// After:
let substs = lower_path_generic_args(path, &mut 0);
let struct_ty = Ty::new(TyKind::Adt(def_id, substs.clone()), expr.span);
AggregateKind::Adt(def_id, 0, substs, field_tys)
```

**Site 4 (line ~1871)** — Enum struct variant: same pattern as Site 3.

**Site 5 (line ~466)** — Fall-through ADT ctor path:
```rust
// Before:
let adt_ty = Ty::new(TyKind::Adt(def_id, Vec::new().into()), expr.span);

// After:
let substs = lower_path_generic_args(path, &mut 0);
let adt_ty = Ty::new(TyKind::Adt(def_id, substs.clone()), expr.span);
```

### 3.2 `src/typeck/unify.rs` — Adt Arm Reworked

```rust
// Before (Stage 16.51 temporary):
if a_substs.len() == b_substs.len() && !a_substs.is_empty() {
    for (at, bt) in a_substs.iter().zip(b_substs.iter()) {
        self.unify_resolved(at, bt)?;
    }
}

// After (Stage 16.52 principled):
if a_substs.is_empty() || b_substs.is_empty() {
    return Ok(()); // empty substs = unknown, unify by DefId only
}
if a_substs.len() != b_substs.len() {
    return Err(...); // mismatched non-empty substs = type error
}
for (at, bt) in a_substs.iter().zip(b_substs.iter()) {
    self.unify_resolved(at, bt)?;
}
```

## 4. API (§23 Compliant)

No new public API. Stage 16.52 reuses `lower_path_generic_args` from
Stage 16.51 and refactors internal call sites only.

## 5. Test Plan

15 tests in `tests/v0/stage16/plan/stage16_52_aggregate_substs_tests.rs`:

| # | Test | Verifies |
|---|------|----------|
| 1 | `generic_struct_literal_unifies` | `Pair<i32, i32>` annot + `Pair { ... }` literal |
| 2 | `generic_struct_literal_inferred` | Inference case (no annot) |
| 3 | `single_param_generic_struct` | `Box<T>` |
| 4 | `generic_enum_tuple_variant_unifies` | `Opt::Some(42)` with `Opt<i32>` annot |
| 5 | `generic_enum_unit_variant_unifies` | `Opt::None` with `Opt<i32>` annot |
| 6 | `generic_enum_struct_variant_unifies` | `Shape::Circle { r: 1 }` |
| 7 | `generic_enum_return` | `fn make() -> Opt<i32>` |
| 8 | `generic_enum_in_match` | Match scrutinee position |
| 9 | `aggregate_substs_propagated_in_mir` | MIR well-formedness |
| 10 | `type_annotation_substs_in_local_decl` | Local decl has substs |
| 11 | `non_generic_struct_no_regression` | Non-generic still works |
| 12 | `non_generic_enum_no_regression` | Non-generic enum still works |
| 13 | `non_generic_enum_with_data_no_regression` | Non-generic enum w/ data |
| 14 | `empty_substs_unify_with_non_empty` | Inference edge case |
| 15 | `document_substs_mismatch_intent` | Forward-looking doc test |

## 6. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt --check` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 250/250 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2452/2452 PASS (+15 new)
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 7926 tests passing, 0 failures, 0 warnings.**

## 7. Version Policy

v0.237.0 → v0.238.0 (minor bump — typeck behavior change: substs are now
compared element-wise when both sides are non-empty. This is a stricter
rule than the Stage 16.51 relaxation, but it's the principled rule that
Phase 2 (substitution) will build on.)

## 8. Next Steps (Task 11 Roadmap)

| Phase | Status | Description |
|-------|--------|-------------|
| 1a | ✅ Stage 16.50 | `generics_of` query |
| 1b | ✅ Stage 16.51 | Substs propagation into `TyKind::Adt` |
| 1c | ✅ Stage 16.52 | Substs propagation into `AggregateKind::Adt` |
| 2 | 🔧 Next | `substitute(ty, substs)` function |
| 3 | 🔧 Planned | Monomorphization collection (`collect_mono_items`) |
| 4 | 🔧 Planned | Per-mono codegen |

## 9. References

- Stage 16.51 design: `docs/develop/v0/stage-16/stage-16.51-substs-propagation.md`
- Stage 16.50 design: `docs/develop/v0/stage-16/stage-16.50-generics-of-query.md`
- Task 11 design: `docs/develop/v0/task-11-monomorphization-design.md`
- Type system data flow: `docs/graph/type-system/data-flow.md`
- Stage Committee process: `docs/stage-committee-process.md` §13.4 + §23
