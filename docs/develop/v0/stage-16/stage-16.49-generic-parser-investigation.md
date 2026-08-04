# Stage 16.49 — Generic Parser Support Investigation: Foundation for Task 11

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.235.2 → v0.236.0
> **Process**: stage-committee-process.md v3.24 §13.4 (stage-start design alignment)

## 1. Executive Summary

Stage 16.49 investigates the current state of generic syntax support across
the compiler pipeline and creates the design document for Task 11
(monomorphization). The investigation reveals that **generic syntax parsing
is fully implemented** (parser → AST → HIR → MIR type slots), but the
**MIR lowerer discards parsed generic args** — always emitting empty
`SubstsRef`. The plumbing exists; the data flow is severed at the HIR→MIR
boundary.

**No code changes** — investigation + design document only.

## 2. Investigation Findings

### 2.1 Parser ✅ Fully Implemented

- `parse_generics()` — parses `<T, 'a, T: Bound, T = Default>`
- `try_parse_generic_args()` — parses `<T, 'a, Item=i32>` after path segment
- `try_parse_turbofish_or_generic_args()` — detects `::<` and delegates
- Wired into all 6 item kinds (fn/struct/enum/trait/impl/type-alias)
- Known limitation: `>>` not split (uses "consume Shr as both closes" hack)

### 2.2 AST ✅ Full Generic AST Types

All types exist: `Generics`, `GenericParam`, `TypeParam`, `TypeBound`,
`TraitBound`, `WherePredicate`, `PathSegment { args: Option<GenericArgs> }`,
`GenericArg::{Lifetime, Type, Assoc}`, `GenericArgs::{AngleBracketed, Parenthesized}`.

### 2.3 HIR ✅ Full HIR Generics

All types exist: `HirGenerics`, `HirGenericParam`, `HirTypeParam`,
`HirTypeBound`, `HirTraitBound`, `HirWherePredicate`.
`HirPathSegment { args: Option<GenericArgs> }` preserves args via clone.
All HIR item kinds carry `generics: HirGenerics`.

### 2.4 MIR Type System ✅ Full Generic Slots

```rust
pub type SubstsRef = Rc<[Ty]>;
pub enum TyKind {
    Adt(DefId, SubstsRef),      // ← substs always empty!
    FnDef(DefId, SubstsRef),    // ← substs always empty!
    Closure(DefId, SubstsRef),  // ← substs = captures (populated!)
    Param(ParamTy),             // ← type parameter T
}
```

### 2.5 🔴 THE CRITICAL GAP — MIR Lower Discards Generic Args

`src/mir/lower/mod.rs:1739-1746`:
```rust
HirTyKind::Path(_, path) => match path.res {
    Res::Def(def_id, _) => Ty::new(
        TyKind::Adt(def_id, Vec::<Ty>::new().into()),  // ← empty substs!
        span,
    ),
```

`path.segments[*].args` is never inspected. Every `Vec<i32>` becomes
`Adt(Vec_def_id, [])` in MIR.

## 3. What's Missing for Basic `Vec<T>` Support

| # | Gap | Location |
|---|-----|----------|
| a | Propagate `path.segments.last().args` into `SubstsRef` | `mir/lower/mod.rs:1739` |
| b | Same for `AggregateKind::Adt` (struct/enum construction) | `mir/lower/expr_operand.rs` |
| c | `generics_of(DefId) -> &[ParamTy]` query | new file |
| d | `substitute(ty, substs)` — replace `Param` with actual types | `mir/ty.rs` |
| e | Monomorphization collection pass | new file |
| f | Per-mono layout/codegen — substitute substs into field types | `codegen/mir_translation.rs` |
| g | Per-mono function codegen — emit one function per `MonoItem` | `codegen/rvalue.rs` |
| h | Resolver validation — validate arg count vs param count | `resolve/path_resolve.rs` |

## 4. Implementation Plan (Task 11)

### Phase 1: Propagation (Steps a-d)
1. Create `generics_of` query — DefId → generics params map
2. Modify `lower_hir_ty_to_mir_ty` to propagate `path.args` into `SubstsRef`
3. Implement `substitute(ty, substs)` — pure function
4. Add tests: `let x: Vec<i32>` produces `Adt(Vec_def_id, [i32])`

### Phase 2: Monomorphization Collection (Step e)
5. Create `src/mir/monomorphize.rs` — walk MIR bodies, collect `MonoItem`s
6. Dedup by `(DefId, SubstsRef)`
7. Add tests: `Vec<i32>` + `Vec<bool>` → 2 MonoItems

### Phase 3: Per-Mono Codegen (Steps f-g)
8. Modify `AdtLayouts` to support per-mono layouts (keyed by `(DefId, SubstsRef)`)
9. Modify codegen to emit one function per `MonoItem`
10. Add tests: `Vec<i32>` and `Vec<bool>` produce different LLVM types

### Phase 4: Validation (Step h)
11. Add resolver validation for generic arg count
12. Add typeck support for `Param` substitution

## 5. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 244/244 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2431/2431 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 7899 tests passing, 0 failures, 0 warnings.**

## 6. Version Policy

v0.235.2 → v0.236.0 (minor bump — new design document for Task 11,
investigation stage marking the start of the generic support workstream.)
