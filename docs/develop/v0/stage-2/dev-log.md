# Stage 2 Development Log

> **Author**: redskaber
> **Date**: 2026-07-22
> **Version**: v0.9.1 (Stage 3.63-3.68 + Stage 4.1-4.4 retroactive updates)
> **Status**: ✅ Complete
> **Test count**: 170 tests (Stage 2 scope)

## Overview

Stage 2 covers MIR (Mid-level Intermediate Representation) types, HIR→MIR
lowering, type checking (inference + unification), and borrow checking
(NLL). The Stage 2 work was originally completed across sub-stages 2.1-2.4
with 6 rounds of gate review, then received retroactive improvements during
Stage 3.63-3.68 (cross-stage naming standardization + P2 fixes) and
Stage 4.4 (closure lowering).

## Sub-stages

### Stage 2.1 — MIR Types + HIR→MIR Lowering

**Version**: v0.3.0-0.3.1

**Work completed**:
- MIR body: `MirBody` (basic_blocks, local_decls, span, adt_layouts),
  `BasicBlock`/`BasicBlockId`, `LocalDecl`, `Statement`/`StatementKind`
  (Assign/Nop/StorageLive/StorageDead/Deinit), `Terminator` (Goto/
  SwitchInt/Return/Unreachable/Drop/Call/Assert), `AssertMessage`
- MIR place (formerly lvalue): `Place`/`PlaceKind` (Local/Static/
  Projection), `ProjectionElem` (Deref/Field/Index/ConstantIndex/
  Subslice), `Operand` (Copy/Move/Constant), `Rvalue` (Use/BinaryOp/
  UnaryOp/Ref/Cast/Aggregate/BinaryOp2[Range])
- MIR types: `Ty`/`TyKind` (16 variants), `Mutability`, `Region`,
  `Sig`, `SubstsRef`, `Const`/`ConstVal`, `InferVar` (TyVar/IntVar/
  FloatVar)
- `lower_hir_body_to_mir` + `MirLowerCtxt` + `lower_body` aliases
  (Stage 3.65)
- 58 mir_lowering tests + 5 inline

**Retroactive updates**:
- **Stage 3.63**: `lower_body` + `lower_body_full` convenience aliases
- **Stage 3.66**: `Lvalue` → `Place` rename (167+ refs); `LvalueKind` →
  `PlaceKind`; file `src/mir/lvalue.rs` → `src/mir/place.rs`
- **Stage 3.66**: `BorrowKind` unified (single source in `mir::place`;
  removed duplicate in `borrowck::borrow_set` + `BkKind` alias)
- **Stage 4.4**: `HirExprKind::Closure` lowering with `AggregateKind::Closure`
  + `TyKind::Closure` (L3 closure lowering groundwork)

### Stage 2.2 — Type Checking (Inference + Unification)

**Version**: v0.3.2-0.3.3

**Work completed**:
- `TypeChecker` struct with `UnificationTable` (TyVar/IntVar/FloatVar
  with union-find + `Linked` pointers)
- `TypeckResults` (local_types, hir_types)
- `TypeError` type
- `check_mir_body_with_tables` (§16-compliant entry point — reads zero HIR;
  receives `FieldTyTable` built by driver)
- `FieldTyTable` + `FnSigTable` (Stage 3.60 — pre-computed data tables
  eliminate typeck→HIR leak)
- Phase 1: walk basic blocks, collect constraints
- Phase 2: default unresolved int/float variables
- Phase 3: write resolved types back into local_decls
- Phase 3.5: writeback field types using pre-computed table
- Phase 3.6: writeback field load locals (Stage 3.36 L-DEBT-3 fix)
- Phase 4: populate TypeckResults
- Phase 5: post-defaulting terminator check
- Coercion matrix (Stage 3.59): widening + Int↔Uint same width +
  Bool→Int + f32→f64
- 26 typeck tests + 12 inline

**Retroactive updates**:
- **Stage 3.60**: §16 compliance — `check_mir_body_with_tables` takes
  `Option<&FieldTyTable>` instead of `&HirCrate`; `fn_sigs` field pub
  so driver can set directly from `FnSigTable`
- **Stage 3.62**: Dead code cleanup — removed ~387 lines of old HIR-
  reading methods (`populate_fn_sigs`, `check_mir_body_with_hir`,
  `writeback_field_load_locals` (HIR version), `writeback_field_types`
  (HIR version), `check_crate` stub)
- **Stage 3.63**: `check_crate` + `check_mir_body_with_hir` marked
  `#[deprecated]`; `typeck/mod.rs` doc updated to point to canonical
  `check_mir_body_with_tables`

### Stage 2.3 — Borrow Checking (NLL)

**Version**: v0.4.0

**Work completed**:
- `BorrowChecker` struct with `BorrowSet` + `MoveTracker` + `initialized`
  set
- Field-sensitive `PlacePath` (Local + Vec<ProjElem>) + `overlaps` check
- NLL (Non-Lexical Lifetimes): single-pass forward with pre-computed
  last-use map
- `BorrowError`/`BorrowErrorKind` (8 variants)
- Borrow expiry at last use (NLL)
- Copy-ness check via typeck results
- G5 fix: mutability check (assign to immutable after init)
- G7 fix: `&mut x` requires x mutable
- Short-circuit && / || lowering (5 BBs)
- StorageLive/StorageDead/Deinit emission
- Assert terminator for arithmetic overflow checks
- 26 borrowck inline tests

**Known limitations**:
- NLL is single-pass forward, not full fixpoint dataflow — borrows used
  inside loops where the borrow was created outside the loop may produce
  false positives (deferred to Stage 4+)
- `ty_is_copy` conservatively treats all Adt types as non-Copy (Stage 3.40
  pragmatically changed to treat Adt as Copy — proper Copy trait is Stage 5)

### Stage 2.4 — Gate Review + P0/P1 Fixes

**Version**: v0.4.0 (6 rounds of review)

**Work completed**:
- Fixed all 17 P0 blockers from Stage 2.x gate review
- Fixed 6 of 8 P1 issues (remaining 2: TraitResolver, region inference —
  Stage 3+)
- 52 new tests (4 union-find + 2 writeback + 10 driver + 3 field-sensitivity
  + 5 ty_is_copy + 2 NLL + 26 integration)
- Total test count: 541 → 593 (+52)

**P0 fixes committed**:
- P0-3: Array length lowering (literal const-eval)
- P0-5: Path for Res::Def → FnDef-typed operand
- P0-6: Deref as Projection (not BitNot)
- P0-9: Union-find propagation (Linked pointers for IntVar/FloatVar)
- P0-12: Type writeback to local_decls (check_mir_body takes &mut)
- P0-13: Driver wiring lexer→parser→hir→resolve→mir→typeck→borrowck
- P0-14: Single-pass NLL with pre-computed last-use map
- P0-15: Field-sensitive PlacePath (Local + Vec<ProjElem> + overlaps)
- P0-16: Borrow expiry at last use (NLL)
- P0-17: Copy-ness check via typeck results

**P1 fixes committed**:
- P1-1: Short-circuit And/Or (lower_short_circuit with 5 BBs)
- P1-2: String/byte literals typed correctly (Str/Slice(u8)/U8)
- P1-3: TypeckResults struct exposed via driver
- P1-4: User-facing error display with source snippets
- P1-5: StorageLive/StorageDead/Deinit StatementKinds + emission
- P1-6: Assert terminator emitted for arithmetic overflow checks

## Key Design Decisions

### 1. §16 Interface Isolation (Stage 3.56-3.60)
- codegen is a pure MIR consumer — zero upstream function calls
- typeck active path (`check_mir_body_with_tables`) reads zero HIR —
  receives `FieldTyTable` + `FnSigTable` as data
- driver is the sole HIR reader (orchestrator)
- All metadata pre-computed: `body_metas`, `fn_name_by_def_id`,
  `FieldTyTable`, `FnSigTable`

### 2. `Place` (formerly `Lvalue`) Naming (Stage 3.66)
- Renamed from `Lvalue` (legacy rustc pre-RFC-1211) to `Place`
- Aligns with design doc 06-mir.md §4 + borrowck internal vocabulary
  (`PlacePath`, `PlaceRoot`)
- 167+ references across 7+ files updated

### 3. `BorrowKind` Unification (Stage 3.63)
- Single source of truth in `mir::place::BorrowKind`
- Removed duplicate in `borrowck::borrow_set::BorrowKind` + `BkKind` alias
- Eliminated 6-line manual conversion code

### 4. Closure Lowering (Stage 4.4)
- `HirExprKind::Closure` → `AggregateKind::Closure(def_id, substs)`
- `TyKind::Closure(def_id, substs)` → `EmitType::Struct(vec![])` in codegen
- Capture analysis deferred to Stage 4.5 (empty environment for now)

## Test Summary

| Test file | Count | Scope |
|-----------|-------|-------|
| `tests/v0/stage2/plan/mir_lowering_tests.rs` | 22 | HIR→MIR lowering + closure (Stage 4.4) |
| `tests/v0/stage2/plan/typeck_tests.rs` | 26 | Type inference + unification + coercion |
| `tests/v0/stage2/plan/integration_tests.rs` | 20 | End-to-end pipeline |
| Inline lib tests | 102 | body.rs + place.rs + ty.rs + checker.rs + unify.rs + borrowck |
| **Total** | **170** | |

## Known Limitations (deferred to Stage 4+)

- **NLL fixpoint dataflow**: single-pass forward, false-positives on
  borrows created outside loop with last use inside loop
- **TraitResolver**: absent (manual `ty_is_copy` workaround treats Adt
  as Copy) — Stage 5
- **Region inference**: placeholder (all `'r → Region::Var(0)`) — Stage 4+
- **Closure capture analysis**: empty environment (Stage 4.5)

---

**Last updated**: 2026-07-22 (Stage 4.4)
**Process version**: v3.16
