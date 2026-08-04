# Stage 16.16 — Task 10 Steps 3+4: Call Site Migration + Codegen (Attempt + Revert)

> **Author**: redskaber
> **Date**: 2026-08-03
> **Version**: v0.228.2 → v0.228.3
> **Process**: stage-committee-process.md v3.24 §1.0 原則 9 "正确 > 妥协" + §23 API 命名标准化

## 1. Executive Summary

Stage 16.16 attempted the "big switch" from inline closure calls to
synthesized `call` functions (Task 10 Steps 3+4). The MIR lowering
side was implemented successfully, but the codegen side revealed
complexity issues that require more work. The switch was reverted to
preserve all tests passing.

**Key changes**:
1. Added `lower_closure_call_to_synthesized()` — emits `TerminatorKind::Call`
   to synthesized closure function (retained, `#[allow(dead_code)]`).
2. Added `codegen_synthesized_closure_functions()` — emits LLVM functions
   for synthesized closure MIR bodies (retained, working but needs refinement).
3. Added `fn_name_by_def_id` registration for synthesized closure functions.
4. Added `fn_sigs` registration for synthesized closure functions.
5. **Reverted** call site to use `lower_closure_call_inline` (inline path).
6. +0 tests (infrastructure retained, switch deferred).

**Result**: Infrastructure is in place. The switch requires fixing codegen
issues (correct BodyMeta, self param handling, return type resolution).
All 7695 tests pass with the inline path.

## 2. The Attempt

### 2.1 MIR Lowering (Successful)

`lower_closure_call_to_synthesized()` was implemented:
- Extracts the closure DefId from `TyKind::Closure(def_id, ...)`
- Creates a FnDef-typed constant pointing to the synthesized function
- Emits `TerminatorKind::Call` with `[self, args...]` as arguments
- The call resolves to the synthesized function via `fn_name_by_def_id`

This part works correctly — the MIR is valid.

### 2.2 Codegen (Issues Found)

`codegen_synthesized_closure_functions()` was added to emit LLVM functions
for `synthesized_closure_mir_bodies`. However, several issues were found:

1. **BodyMeta synthesis**: The synthesized MIR bodies don't have BodyMeta
   entries. The codegen function synthesizes them, but the `param_count`
   and `is_void` values are incorrect.

2. **Self parameter handling**: The `self` parameter (LocalId(1)) is the
   closure struct, which should be passed as a pointer. But codegen
   treats it as a regular i32 parameter.

3. **Return type**: The return type defaults to `{}` (unit) instead of
   the actual return type (e.g., i32). The `fn_sigs` registration uses
   a placeholder Infer type that codegen can't resolve.

4. **Function name resolution**: The codegen function searches for
   `closure_call_fn_` in `fn_name_by_def_id`, which is fragile (finds
   the first match, not the correct one for each MirBody).

### 2.3 Revert

The switch was reverted to `lower_closure_call_inline` to preserve all
tests passing. The infrastructure (functions, registration, codegen
function) is retained for future use.

## 3. What Was Learned

The codegen for synthesized closure functions needs:

1. **Proper BodyMeta**: Each synthesized MIR body needs a BodyMeta with
   correct `param_count` (self + closure params), `is_void` (false for
   closures that return values), and `abi` (Landin).

2. **Self parameter as pointer**: The `self` parameter should be emitted
   as a pointer type (OpaquePtr), not i32.

3. **Return type from MIR body**: The return type should come from the
   MIR body's LocalId(0) type (after typeck writeback), not from a
   placeholder fn_sig.

4. **DefId on MirBody**: The fragile name lookup would be fixed by
   adding a `def_id` field to `MirBody` (or a separate map from MirBody
   index to DefId).

## 4. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 244/244 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2227/2227 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 7695 tests passing, 0 failures, 0 warnings.**

## 5. Version Policy

v0.228.2 → v0.228.3 (patch bump — infrastructure added, switch reverted,
no behavior change.)

## 6. Task 10 Roadmap

| Step | Status | Description |
|------|--------|-------------|
| Step 1 | ✅ COMPLETE (Stage 16.13) | Infrastructure |
| Step 2 | ✅ COMPLETE (Stage 16.14) | MIR body synthesis |
| Step 3 | 🔧 Partial (Stage 16.16) | Call site migration — MIR side done, codegen needs work |
| Step 4 | 🔧 Partial (Stage 16.16) | Codegen — function added, needs refinement |
| Step 5 | 🔧 Pending | Cleanup |

**Next**: Fix codegen issues (BodyMeta, self param, return type) then
re-attempt the switch.
