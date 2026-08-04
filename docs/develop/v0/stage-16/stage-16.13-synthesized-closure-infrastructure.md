# Stage 16.13 — Task 10 Step 1: Synthesized Closure Function Infrastructure

> **Author**: redskaber
> **Date**: 2026-08-03
> **Version**: v0.227.6 → v0.228.0
> **Process**: stage-committee-process.md v3.24 §1.0 原則 6 "通用 > 特例" + §13.4 (数据结构选型) + §16 (接口隔离) + §23 API 命名标准化

## 1. Executive Summary

Stage 16.13 is **Step 1 of Task 10** (Closure Redesign). It adds the
infrastructure for Strategy A (rustc-style synthesized `call` function
per closure) without changing the current inline behavior.

**Key changes**:
1. Added `SynthesizedClosureFunction` struct to `mir::lower`.
2. Added `synthesized_closure_functions` side-table to `MirLowerCtxt`.
3. Added `allocate_closure_def_id()` method — allocates unique DefIds
   from a reserved range (was: all closures shared the crate's first
   owner DefId — incorrect).
4. Added `register_synthesized_closure_function()` method.
5. Updated closure literal lowering to register synthesized functions.
6. Updated `lower_hir_body_to_mir_full*` return type to include the
   side-table (4-tuple now, was 3-tuple).
7. Created Task 10 design document.
8. +8 integration tests.

**Result**: Infrastructure in place for Strategy A. No behavior change —
the inline approach (Stage 13.3a) is still used for closure calls.

## 2. Background

### 2.1 Current State (Stage 13.3a — Inline)

The current closure call lowering inlines the closure body at each call
site. This works but has limitations:
- Code bloat (body copied at each call site)
- No optimization opportunity (LLVM can't deduplicate)
- MIR pollution (body locals in enclosing function)
- Doesn't match the design (`07-codegen.md` §8.1-8.2 prescribes Strategy A)

### 2.2 Target (Strategy A — Synthesized `call` Function)

Each closure literal generates:
1. An anonymous struct holding captures (already exists)
2. A synthesized `call` function: `extern "Landin" fn call(&self, args...) -> ret`
3. Call site `f(42)` lowers to `TerminatorKind::Call` to the synthesized function

### 2.3 Step 1 Scope

Step 1 (this stage) adds the infrastructure:
- `SynthesizedClosureFunction` struct with all metadata
- Side-table on `MirLowerCtxt` for collecting during lowering
- Unique DefId allocation (fixes the shared-DefId bug)
- Return path from lower to driver (side-table returned)

**No behavior change** — the inline approach is still used. Step 2+
will build the synthesized MIR body and migrate call sites.

## 3. Implementation

### 3.1 `SynthesizedClosureFunction` Struct

```rust
#[derive(Clone, Debug)]
pub struct SynthesizedClosureFunction {
    pub def_id: DefId,
    pub params: Vec<HirParam>,
    pub body: Box<HirExpr>,
    pub captures: Vec<(HirId, u32, Ty)>,  // (hir_id, field_index, field_type)
    pub closure_struct_ty: Ty,
    pub fn_name: String,  // e.g., "closure_call_fn_0"
}
```

### 3.2 DefId Allocation

Before Stage 16.13, all closures in a crate shared the crate's first
owner DefId — incorrect. Stage 16.13 allocates unique DefIds from a
reserved range:

```rust
pub fn allocate_closure_def_id(&mut self) -> DefId {
    const CLOSURE_DEF_ID_BASE: u32 = u32::MAX - 1000;
    let id = CLOSURE_DEF_ID_BASE - self.closure_def_id_counter;
    self.closure_def_id_counter += 1;
    DefId::new(id)
}
```

This matches the pattern of `BUILTIN_DEF_ID_BASE` (Stage 5.8) for
builtin traits. The range `u32::MAX - 1000` downward avoids collision
with builtin traits (u32::MAX downward) and user items (low DefIds).

### 3.3 Side-Table Registration

During closure literal lowering, the synthesized function metadata is
registered:

```rust
let synthesized_func = SynthesizedClosureFunction {
    def_id: closure_def_id,
    params: params.clone(),
    body: body.clone(),
    captures: synthesized_captures,
    closure_struct_ty: closure_ty.clone(),
    fn_name: format!("closure_call_fn_{}", cx.closure_def_id_counter - 1),
};
cx.register_synthesized_closure_function(synthesized_func);
```

### 3.4 Return Type Change

`lower_hir_body_to_mir_full*` now returns a 4-tuple:
```rust
(MirBody, UnificationTable, Vec<TypeError>, HashMap<DefId, SynthesizedClosureFunction>)
```

The driver and test callers were updated to destructure the 4th element
(prefix `_synthesized_closures` — not yet used by codegen).

## 4. API Naming Standard Compliance (§23)

| Item | Pattern | Status |
|------|---------|--------|
| `SynthesizedClosureFunction` | `<Adj><Noun>` | ✅ |
| `synthesized_closure_functions` | `<adj>_<noun>_<noun>` | ✅ |
| `allocate_closure_def_id` | `<verb>_<noun>_<noun>` | ✅ |
| `register_synthesized_closure_function` | `<verb>_<adj>_<noun>_<noun>` | ✅ |
| `closure_call_fn_{n}` | `<noun>_<verb>_<noun>_{n}` | ✅ |

## 5. §16 Interface Isolation Compliance

- `MirLowerCtxt` builds the side-table during lowering (reads HIR — allowed)
- Driver receives the side-table but doesn't use it yet (prefixed `_`)
- Codegen will read the side-table in Step 4 (future)
- No new HIR access from borrowck/codegen

## 6. Tests

Added `tests/v0/stage16/plan/stage16_13_synthesized_closure_infrastructure_tests.rs`
with 8 tests:
1. Closure literal registers synthesized function
2. Multiple closures get unique DefIds
3. Closure with captures compiles
4. Closure without captures compiles
5. Nested closures compile
6. Closure call produces correct result (inline path)
7. Closure with multiple params compiles
8. Closures in different functions get different DefIds

All tests verify compilation succeeds (no errors). Direct side-table
inspection is deferred to Step 2 when the driver exposes it.

## 7. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 244/244 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2211/2211 PASS (+8 new)
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 7679 tests passing, 0 failures, 0 warnings.**

## 8. Version Policy

v0.227.6 → v0.228.0 (**minor bump** — return type change is a breaking
API change for `lower_hir_body_to_mir_full*`. The `SynthesizedClosureFunction`
struct is new public API. No behavior change for valid programs, but
the API surface changed.)

## 9. Task 10 Roadmap

| Step | Status | Description |
|------|--------|-------------|
| Step 1 | ✅ COMPLETE (Stage 16.13) | Infrastructure: struct, side-table, DefId allocation |
| Step 2 | 🔧 Pending | MIR body synthesis for `call` function |
| Step 3 | 🔧 Pending | Call site migration (inline → `TerminatorKind::Call`) |
| Step 4 | 🔧 Pending | Codegen: emit LLVM function |
| Step 5 | 🔧 Pending | Cleanup: remove `ClosureBodyInfo`, inline path |

**Next**: Step 2 (MIR body synthesis) — build a separate MIR body for
each synthesized closure function, stored in `CompileResult`.
