# Stage 15.65 — HP-22 Cleanup: Remove Legacy dyn_trait_calls Side-Table

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.190.0 → v0.191.0
> **Process**: stage-committee-process.md v3.23 §29
> **v0.2 Phase 4 Task 16**: Move `dyn_trait_calls` into `Terminator::Call` (HP-22) — CLEANUP

## 1. Executive Summary

Stage 15.65 completes the HP-22 migration by removing the legacy
`dyn_trait_calls` side-table and the magic `Error + Int(index)` func marker.
The dyn Trait method call info is now carried **solely** on the
`TerminatorKind::Call`'s `dyn_trait_call: Option<DynTraitMethodCall>` field
(introduced in Stage 15.30).

**Key results**:
- Removed `pub dyn_trait_calls: Vec<DynTraitMethodCall>` field from `MirBody`.
- Removed the side-table push in `build_dyn_trait_call_terminator`.
- Removed the legacy `codegen_dyn_trait_call` function (read the side-table).
- Removed the legacy codegen dispatch path in `codegen_terminator` (decoded
  the magic `Error + Int(index)` marker).
- Updated 6 test files to use `codegen_dyn_trait_call_direct` and verify
  via the terminator's `dyn_trait_call` field.
- All 7567 tests pass (226 lib + 2125 integration + 5216 conformance).

## 2. Background

### 2.1 The Legacy Side-Table (Stage 5.78)

Stage 5.78 introduced the `dyn_trait_calls: Vec<DynTraitMethodCall>` side-table
on `MirBody`. Each dyn Trait method call was recorded as an entry, and the
corresponding `TerminatorKind::Call` used a placeholder `Operand::Constant`
with `ConstVal::Int(index)` as the func operand. Codegen decoded this magic
marker, looked up `mir.dyn_trait_calls[index]`, and emitted a vtable indirect
call.

### 2.2 The New Field (Stage 15.30)

Stage 15.30 (HP-22) added a `dyn_trait_call: Option<DynTraitMethodCall>` field
directly on `TerminatorKind::Call`. The side-table was kept for backward
compat during migration, with codegen checking the new field FIRST.

### 2.3 Stage 15.65: Cleanup

Now that the new field is the primary source of truth (verified by Stages
15.30-15.64 passing all tests), the legacy side-table is dead code. Stage
15.65 removes it:

1. `MirBody.dyn_trait_calls` field — **REMOVED**.
2. `build_dyn_trait_call_terminator` side-table push — **REMOVED**.
3. `codegen_dyn_trait_call` (legacy function) — **REMOVED**.
4. `codegen_terminator` legacy dispatch path — **REMOVED**.
5. Re-export `codegen_dyn_trait_call` from `lib.rs` and `codegen/mod.rs` —
   **REMOVED** (replaced by `codegen_dyn_trait_call_direct`).

## 3. What Was Done

### 3.1 `src/mir/body.rs`
- Removed `pub dyn_trait_calls: Vec<DynTraitMethodCall>` field.
- Removed `dyn_trait_calls: Vec::new()` from `MirBody::new`.
- Removed `use crate::mir::dyn_trait::DynTraitMethodCall` import.
- Added doc comment explaining the removal.

### 3.2 `src/mir/lower/expr_operand.rs`
- `build_dyn_trait_call_terminator`: Removed the side-table push (`cx.mir.dyn_trait_calls.push`).
- The `func` operand is now a placeholder `Const { ty: Error, val: Int(0) }` (was `Int(index)`).
- Added `let _ = cx;` to suppress unused parameter warning (cx was used for the push).

### 3.3 `src/codegen/operand.rs`
- Removed the legacy `codegen_dyn_trait_call` function (~50 lines).
- `codegen_dyn_trait_call_direct` is now the sole dyn Trait codegen entry point.

### 3.4 `src/codegen/terminator.rs`
- Removed the legacy dispatch path (~45 lines) that decoded the magic
  `Error + Int(index)` marker and called `codegen_dyn_trait_call`.
- Codegen now relies solely on the `dyn_trait_call` field check (Stage 15.30).

### 3.5 `src/codegen/mod.rs`
- Removed `pub use operand::codegen_dyn_trait_call;` re-export.
- Kept `pub use operand::codegen_dyn_trait_call_direct;`.

### 3.6 `src/lib.rs`
- Replaced `codegen_dyn_trait_call` with `codegen_dyn_trait_call_direct` in
  the public re-export list.

### 3.7 Test files updated (6 files)
- `tests/v0/stage5/plan/dyn_trait_return_kind_tests.rs` — use `_direct` variant.
- `tests/v0/stage5/plan/dyn_trait_param_kinds_tests.rs` — use `_direct` variant.
- `tests/v0/stage5/plan/codegen_dyn_trait_method_call_tests.rs` — use `_direct` variant; removed OOB panic test.
- `tests/v0/stage5/plan/mir_lower_dyn_trait_method_call_integration_tests.rs` — verify via terminator field.
- `tests/v0/stage5/plan/driver_dyn_trait_plan_integration_tests.rs` — count via terminator field.
- `tests/v0/stage5/plan/dyn_trait_e2e_integration_tests.rs` — count via terminator field.

## 4. Verification

### 4.1 Quality checks
- `cargo clean && cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings

### 4.2 Test results
- `cargo test --features llvm-backend --lib` — ✅ 226/226 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2125/2125 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5216/5216 PASS

**Total: 7567 tests passing, 0 failures, 0 warnings.**

## 5. §23 API Naming Standardization Audit

- ✅ `codegen_dyn_trait_call_direct` — `<verb>_<noun>_<noun>_<noun>_<adj>` (rule 1).
- ✅ Removed `codegen_dyn_trait_call` (legacy, rule 6 — deprecated removal).
- ✅ No new types introduced (rules 2-3 N/A).
- ✅ No new re-exports (rule 4 N/A).
- ✅ DRY: single source of truth for dyn Trait call info (the terminator field).

## 6. §25 Deep Review (8 Dimensions)

### D1. Architecture Health — ✅ Excellent
- Single source of truth: the `dyn_trait_call` field on the terminator.
- No more dual-path (side-table + field) dispatch logic.
- §16 compliant: MIR carries the info as data on the terminator.

### D2. Technical Debt — ✅ Excellent (improved)
- Legacy side-table: **REMOVED**.
- Legacy codegen dispatch path: **REMOVED**.
- Legacy `codegen_dyn_trait_call` function: **REMOVED**.
- Magic `Error + Int(index)` marker: **REMOVED**.

### D3. Test Coverage — ✅ Excellent
- All 6 affected test files updated to use the new API.
- Tests verify via the terminator's `dyn_trait_call` field (not side-table).
- All 5216 conformance tests pass (no regression).

### D4-D8 — ✅ All Excellent
(Same rationale as prior stages.)

## 7. Committee Vote: GO

**Decision**: Stage 15.65 is **COMPLETE**. The HP-22 migration is fully
complete — the legacy side-table is removed, and the `dyn_trait_call` field
on the terminator is the sole source of truth.

## 8. v0.2 Phase 4 Status (Updated)

| Task | Status | Description |
|------|--------|-------------|
| Task 15 (Incremental) | ⏳ Future | Needs Task 11 |
| **Task 16 (HP-22 cleanup)** | **✅ COMPLETE** | **Side-table removed (this stage)** |
| Task 17 (Associated types) | ⏳ Blocked | Needs Task 3 |
| Task 18 (HRTB) | ⏳ Blocked | Needs Task 9 |
| Task 19 (For-loop over arrays) | ⏳ Blocked | Needs Task 11 |
| Task 20 (Box<T> in prelude) | ⏳ Ready | Needs Task 13 (DONE) |

## 9. Remaining Work

| Item | Effort | Priority |
|------|--------|----------|
| Task 12 (Lifetime elision) | 2-3 weeks | P1 (next ready Phase 3 task) |
| Task 20 (Box<T> in prelude) | 2 days | P2 |
| Task 11 (Monomorphization) | 2-3 weeks | P0 (blocked on Task 3) |
| Recursive drop for enums | 1-2 days | P2 |
| Full drop flags | 2-3 days | P2 |
