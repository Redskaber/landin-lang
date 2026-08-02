# Stage 15.60 — DefId Mismatch Fix (Partial — Crash Persists)

> **Author**: redskaber
> **Date**: 2026-08-01
> **Version**: v0.185.0 → v0.186.0
> **Process**: stage-committee-process.md v3.23 §29
> **v0.2 Phase 3 Task 13**: `impl Drop` + RAII types — DefId fix attempt

## 1. Executive Summary

Stage 15.60 attempts to fix the DefId mismatch identified in Stage 15.59.
The fix was applied (use the type's DefId instead of the impl block's DefId
in `emit_drop_glue_functions`), but the crash persists — the program with
`impl Drop` still crashes during `compile()`.

**Conclusion**: The DefId fix was necessary but not sufficient. The crash
has an additional root cause that requires deeper investigation (likely in
the `elaborate_drops` pass or the `TerminatorKind::Drop` codegen path).
The fix is retained (it's correct) but the crash investigation is deferred
to a future debugging stage.

## 2. What Was Done

### 2.1 Fixed DefId mismatch in `emit_drop_glue_functions`

Changed `src/codegen/mod.rs`:
```rust
// Before (wrong — uses impl block's DefId):
let self_def_id = impl_info.def_id;

// After (correct — uses type's DefId):
let self_def_id = resolver.type_by_def_id.iter()
    .find(|(_, name)| **name == *type_spur)
    .map(|(id, _)| *id)
    .unwrap_or(impl_info.def_id);
```

This ensures `emit_drop_glue_functions` emits `drop_adt_<typeDefId>` which
matches what `TerminatorKind::Drop` codegen calls.

### 2.2 Crash persists

After the fix, the program with `impl Drop` still crashes (exit code 137).
The crash occurs during `compile()` — the `--emit-llvm-ir` flag produces
no output, suggesting the crash happens before codegen output is produced.

**Possible additional root causes**:
1. `elaborate_drops` may produce invalid MIR (e.g., a `Drop` terminator
   with a `target` block that doesn't exist).
2. The `TerminatorKind::Drop` codegen may have a bug in how it computes
   the place address or type.
3. The codegen may crash when processing the `Drop` terminator's block
   splitting (new blocks created by `elaborate_drops` may not have
   proper local declarations).

**Investigation approach** (for future debugging stage):
1. Add debug prints in `elaborate_drops` to see what MIR is produced.
2. Add debug prints in `TerminatorKind::Drop` codegen to see what's called.
3. Try compiling with `--emit-llvm-ir` and check if any IR is produced
   before the crash.
4. Check if the crash is a segfault (accessing invalid memory) or an
   abort (LLVM verification failure).

## 3. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt --check` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 226/226 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5216/5216 PASS (no regression)

## 4. Status

The DefId fix is **retained** (it's correct and necessary). The crash
investigation is **deferred** to a future debugging stage. All existing
tests pass (zero regression) — the crash only affects programs with
`impl Drop`, which no existing test exercises.
