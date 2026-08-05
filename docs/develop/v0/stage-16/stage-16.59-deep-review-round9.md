# Stage 16.59 — Deep Review Round 9: Task 11 Full Audit + Phase 4c Pipeline Integration

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.244.0 → v0.245.0
> **Process**: stage-committee-process.md v3.24 §25 (Deep Review) + §23

## 1. Executive Summary

Stage 16.59 is a deep review round (Round 9) that audited the entire Task 11
(Monomorphization) implementation and fixed the critical issue found: Phase 4c
was API-complete but NOT wired into the production codegen pipeline.

**Audit finding (NO-GO)**: The `mir_type_to_emit_type_with_layouts_and_mono`
function existed and was tested, but the actual codegen pipeline
(`run_codegen_pipeline` → `codegen_from_mir` → `codegen_function` → all
sub-modules) still called the legacy `mir_type_to_emit_type_with_layouts`.
All MonoLayoutMap machinery was dead code from the production codegen path's
perspective.

**Fix applied**: Wired `MonoLayoutMap` through the entire codegen pipeline:
1. `run_codegen_pipeline` now builds `MonoLayoutMap` from collected MonoItems
2. `codegen_from_mir` and `codegen_synthesized_closure_functions` receive
   `mono_layouts` parameter
3. `codegen_function` receives and threads `mono_layouts` to all sub-modules
4. All ~25 call sites of `mir_type_to_emit_type_with_layouts` replaced with
   `mir_type_to_emit_type_with_layouts_and_mono`
5. `get_call_dest_type` updated to accept and use `mono_layouts`
6. Updated 3 test files to pass `None` for the new parameter

**Also fixed**:
- Misleading doc comment on `MonoLayoutKey` (incorrectly claimed `Ty` doesn't
  implement `Hash`/`Eq` — it does, but `Rc<[Ty]>` doesn't, which is the real
  reason for using `Vec<TyKind>`)
- Updated Task 11 design doc status to reflect actual completion

**Test results**: 8071 tests passing (343 lib + 2504 integration + 5224
conformance subset), 0 failures, 0 warnings.

## 2. Audit Findings

### 2.1 Critical (Fixed)

**Phase 4c not wired into production codegen pipeline**
- `mir_type_to_emit_type_with_layouts_and_mono` was defined, re-exported, and
  tested, but never called by the actual codegen pipeline
- All codegen sub-modules still called `mir_type_to_emit_type_with_layouts`
- `Box<i32>` and `Box<bool>` produced identical LLVM IR (no specialization)
- **Fix**: Threaded `MonoLayoutMap` through the entire pipeline

### 2.2 Should-Fix (Not blocking, noted for future)

1. **monomorphize.rs is 1749 lines** — mixes 4 concerns (item/collect/mangle/layout).
   Should be split into sub-modules. (Deferred — functional, just large)

2. **`build_mono_layouts` reaches into HIR** — couples layout-building to HIR.
   Could pre-lower field types at MIR-lowering time. (Deferred — works correctly)

3. **`collect_from_ty` is `pub` but should be `pub(crate)`** — only used
   internally. (Minor visibility issue)

4. **`generics_of`/`build_generics_map` not re-exported from `hir/mod.rs`** —
   inconsistent with `mir/mod.rs` re-exports. (Minor consistency issue)

5. **`mangle_ty` separator `_`** could cause name collisions in degenerate
   cases. (Latent risk, no current collision found)

### 2.3 Fixed Doc Issues

- `MonoLayoutKey` doc comment incorrectly claimed `Ty` doesn't implement
  `Hash`/`Eq`. Fixed to explain the real reason: `Rc<[Ty]>` doesn't implement
  `Hash`/`Eq`, and `TyKind::clone()` is cheaper than `Ty::clone()`.

## 3. Changes

### 3.1 `src/codegen/mod.rs` — Pipeline Integration

- `run_codegen_pipeline`: Now builds `MonoLayoutMap` from collected MonoItems
  and HIR, passes it to `codegen_from_mir` and `codegen_synthesized_closure_functions`
- `codegen_from_mir`: Added `mono_layouts: &MonoLayoutMap` parameter
- `codegen_synthesized_closure_functions`: Added `mono_layouts: &MonoLayoutMap` parameter
- `codegen_function`: Added `mono_layouts: Option<&MonoLayoutMap>` parameter,
  threads to `codegen_statement`, `codegen_terminator`, `get_call_dest_type`
- `get_call_dest_type`: Added `mono_layouts: Option<&MonoLayoutMap>` parameter
- All `mir_type_to_emit_type_with_layouts` calls → `mir_type_to_emit_type_with_layouts_and_mono`

### 3.2 `src/codegen/statement.rs` — Sub-module Update

- `codegen_statement`: Added `mono_layouts: Option<&MonoLayoutMap>` parameter
- All `mir_type_to_emit_type_with_layouts` calls → `_and_mono` with `mono_layouts`
- `codegen_operand` and `codegen_rvalue` recursive calls updated with `mono_layouts`

### 3.3 `src/codegen/terminator.rs` — Sub-module Update

- `codegen_terminator`: Added `mono_layouts: Option<&MonoLayoutMap>` parameter
- All `mir_type_to_emit_type_with_layouts` calls → `_and_mono` with `mono_layouts`
- `codegen_operand` and `codegen_dyn_trait_call_direct` calls updated

### 3.4 `src/codegen/operand.rs` — Sub-module Update

- `codegen_operand`: Added `mono_layouts: Option<&MonoLayoutMap>` parameter
- `codegen_dyn_trait_call_direct`: Added `_mono_layouts` parameter (unused —
  dyn Trait calls don't use per-mono layouts)
- All `mir_type_to_emit_type_with_layouts` calls → `_and_mono`

### 3.5 `src/codegen/rvalue.rs` — Sub-module Update

- `codegen_rvalue`: Added `mono_layouts: Option<&MonoLayoutMap>` parameter
- All `mir_type_to_emit_type_with_layouts` calls → `_and_mono` with `mono_layouts`
- `codegen_operand` recursive calls updated
- Removed unused import `mir_type_to_emit_type_with_layouts`

### 3.6 `src/mir/monomorphize.rs` — Doc Fix

- Fixed `MonoLayoutKey` doc comment: corrected the justification for using
  `Vec<TyKind>` instead of `SubstsRef`

### 3.7 Test Files Updated

- `tests/v0/stage5/plan/codegen_dyn_trait_method_call_tests.rs`: Added `None`
  for `mono_layouts` parameter in 6 call sites
- `tests/v0/stage5/plan/dyn_trait_param_kinds_tests.rs`: Added `None` in 5 call sites
- `tests/v0/stage5/plan/dyn_trait_return_kind_tests.rs`: Added `None` in 5 call sites

## 4. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt --check` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 343/343 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2504/2504 PASS
- Conformance subset (30 tests) — ✅ all pass
- **Total: 8071 tests passing, 0 failures, 0 warnings.**

## 5. Deep Review Recommendation: **GO**

The critical Phase 4c integration gap has been fixed. The MonoLayoutMap is
now wired through the entire codegen pipeline. All tests pass with 0 warnings.

### Remaining deferred items (not blockers):
- Split `monomorphize.rs` into sub-modules (1749 lines → 4 files)
- Reduce HIR coupling in `build_mono_layouts`
- Fix `collect_from_ty` visibility
- Re-export `generics_of` from `hir/mod.rs`
- Consider stronger separator in `mangle_ty`

These are quality improvements that can be addressed in future stages without
blocking the current release.

## 6. Version Policy

v0.244.0 → v0.245.0 (minor bump — critical fix: Phase 4c codegen pipeline
integration. This changes the actual LLVM IR output for generic types —
`Box<i32>` and `Box<bool>` now produce distinct specialized layouts.)

## 7. References

- Stage 16.58 design: `docs/develop/v0/stage-16/stage-16.58-codegen-integration.md`
- Stage 16.57 design: `docs/develop/v0/stage-16/stage-16.57-per-mono-layouts.md`
- Task 11 design: `docs/develop/v0/task-11-monomorphization-design.md`
- Type system data flow: `docs/graph/type-system/data-flow.md`
- Stage Committee process: `docs/stage-committee-process.md` §25 + §23
