# Stage 16.17 — Task 10 Step 3+4: Codegen Fix Attempt (MirBody.def_id)

> **Author**: redskaber
> **Date**: 2026-08-03
> **Version**: v0.228.3 → v0.228.4
> **Process**: stage-committee-process.md v3.24 §1.0 原則 9 "正确 > 妥协" + §23 API 命名标准化

## 1. Executive Summary

Stage 16.17 attempted to fix the codegen issues from Stage 16.16 by
adding a `def_id` field to `MirBody`, enabling proper function name
resolution. The fix improved name resolution but deeper codegen issues
remain (closure struct as pointer, return type resolution). The switch
was reverted again.

**Key changes**:
1. Added `def_id: Option<DefId>` field to `MirBody`.
2. Updated `build_synthesized_closure_mir_body()` to set `def_id`.
3. Updated `codegen_synthesized_closure_functions()` to use `def_id`
   for name resolution (replaces fragile string search).
4. **Reverted** call site to inline path (codegen still needs deeper fixes).
5. +0 tests (infrastructure improved, switch still deferred).

**Result**: `MirBody.def_id` is a permanent improvement — it will be
used by future codegen work. All 7695 tests pass with the inline path.

## 2. The Fix: MirBody.def_id

### 2.1 Problem (Stage 16.16)

The codegen function used a fragile string search to find the function
name for synthesized closure MIR bodies:
```rust
let fn_name = fn_name_by_def_id
    .values()
    .find(|name| name.starts_with("closure_call_fn_"))
    .cloned();
```
This finds the FIRST match, not the correct one for each MirBody.

### 2.2 Solution (Stage 16.17)

Added `def_id: Option<DefId>` to `MirBody`:
```rust
pub struct MirBody {
    // ... existing fields ...
    pub def_id: Option<crate::hir::DefId>,
}
```

Set during `build_synthesized_closure_mir_body()`:
```rust
cx.mir.def_id = Some(func.def_id);
```

Codegen now uses direct lookup:
```rust
let def_id = mir.def_id?;
let fn_name = fn_name_by_def_id.get(&def_id)?;
```

### 2.3 Remaining Issues

Even with correct name resolution, the codegen still produces incorrect
LLVM IR:

1. **Closure struct as pointer**: The `self` parameter should be emitted
   as a pointer (OpaquePtr), but codegen treats it as i32.

2. **Return type**: The return type comes from `mir.local_decls[0].ty`,
   which is a fresh Infer type (never resolved by typeck, since typeck
   doesn't run on synthesized MIR bodies).

3. **Call site arg passing**: The call site passes `{} 0` (unit) as the
   first arg instead of the closure struct pointer.

These require deeper codegen changes:
- Typeck must run on synthesized MIR bodies (to resolve return type)
- Codegen must handle Closure struct types as pointers
- The call site must pass the closure struct by pointer

## 3. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 244/244 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2227/2227 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 7695 tests passing, 0 failures, 0 warnings.**

## 4. Version Policy

v0.228.3 → v0.228.4 (patch bump — MirBody.def_id added, switch still
deferred, no behavior change.)

## 5. Task 10 Roadmap

| Step | Status | Description |
|------|--------|-------------|
| Step 1 | ✅ COMPLETE (16.13) | Infrastructure |
| Step 2 | ✅ COMPLETE (16.14) | MIR body synthesis |
| Step 3 | 🔧 Partial (16.16+16.17) | Call site migration — MIR side done, codegen needs deeper fixes |
| Step 4 | 🔧 Partial (16.16+16.17) | Codegen — DefId resolution fixed, struct/return type needs work |
| Step 5 | 🔧 Pending | Cleanup |

**Next**: The switch requires:
1. Typeck on synthesized MIR bodies (resolve return type)
2. Codegen handling Closure struct as pointer
3. Call site passing closure struct by pointer

These are deeper architectural changes that should be done carefully.
