# Stage 16.29 — 通解: Typeck on Synthesized Closure MIR Bodies

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.229.6 → v0.230.0
> **Process**: stage-committee-process.md v3.24 §1.0 原則 6 "通用 > 特例" + 原則 9 "正确 > 妥协"

## 1. Executive Summary

Stage 16.29 closes the **typeck gap** that forced the `has_complex_captures`
special-case routing (特解) introduced in Stage 16.28. The 通解 (general
solution) shares the unify table between the main body and all closure MIR
bodies, runs typeck on each closure MIR body, and updates fn_sigs with
resolved types.

**Key achievements**:
1. **ALL closures use the synthesized `call` function path** — no more
   `has_complex_captures` special-case routing.
2. **Nested closures compile** (`|| || x` — typeck + borrowck pass).
3. **Closure Copy derivation**: closures with all-Copy captures are Copy
   (mirrors Rust's `#[derive(Copy)]` for closure structs).
4. **`AggregateKind::Closure` returns the actual `Closure(def_id, substs)`
   type** (was: fresh Infer var — caused nested closure type inference to
   fail).
5. **`post_check_terminator` and `check_terminator` accept `Closure` as
   callable** (was: only FnDef/FnPtr — caused false "expected function"
   errors for `f()()` patterns).

**Test results**: 7717 tests passing (244 lib + 2249 integration + 5224
conformance), 0 failures, 0 warnings. Runtime verified: `f(10) = 11` ✅,
`x + y = 15` ✅ (i32 captures).

**Remaining issue**: Nested closure *runtime* (`f()()` where `f` returns a
closure) fails in codegen with "Called function must be a pointer!" — the
codegen doesn't yet handle calling a `Closure`-typed local. This is a
codegen issue, not a typeck/borrowck issue. Tracked as TD-CLOSURE-CODEGEN-1.

## 2. Root Cause Analysis (Stage 16.28 → 16.29)

### 2.1 The Typeck Gap (Stage 16.28 finding)

Stage 16.28 identified that synthesized closure MIR bodies don't run typeck,
so their return type stays `Infer`. This caused type errors for closures
that return non-primitive types (e.g., nested closures, struct captures).

Stage 16.28's workaround: `has_complex_captures` check routes closures with
Adt or Closure captures to the inline path. This is a 特解 (special-case
solution) — it fixes the symptom but not the root cause.

### 2.2 The Real Root Cause: Unify Table Isolation

Stage 16.29 discovered the DEEPER root cause: **unify table isolation**.

When `build_synthesized_closure_mir_body` creates a new `MirLowerCtxt`, it
gets a FRESH `UnificationTable`. But the `closure_struct_ty` and `cap_tys`
(passed in via `SynthesizedClosureFunction`) have `Infer` vars from the
MAIN body's unify table.

These two sets of Infer vars collide — same `TyVid` values (0, 1, etc.)
but from different tables. When typeck runs on the closure MIR body:

1. `closure_struct_ty`'s `Infer(TyVar(0))` is from the main body's table.
2. The closure MIR body's `LocalId(0)` (return local) has `Infer(TyVar(0))`
   from the closure's fresh table.
3. typeck's `unify(TyVar(0), TyVar(0))` sees them as the SAME variable
   (same TyVid), but they're from DIFFERENT tables.
4. The unify table creates a CYCLE: `TyVar(0) → TyVar(1)` and
   `TyVar(1) → TyVar(0)`.
5. `resolve_ty_var` has a depth guard (1024), but `resolve()` re-enters
   `resolve_ty_var` with the returned bound type, resetting the depth
   counter → **infinite recursion → stack overflow**.

### 2.3 The 通解 Fix

**Share the unify table** between the main body and all closure MIR bodies.
This way:
- `closure_struct_ty`'s Infer vars are in the shared table.
- The closure MIR body's fresh Infer vars are allocated from the shared
  table (continuing the TyVid counter).
- No TyVid collision → no cycle → no stack overflow.

The driver flow:
1. Lower main body → `main_mir`, `shared_unify`, `synthesized_closures`
2. For each closure (worklist, handles nested):
   a. `build_synthesized_closure_mir_body(func, interner, hir, shared_unify, counter)`
      → `(closure_mir, shared_unify, errors, nested_closures, counter)`
   b. Register fn_name + placeholder fn_sig (fresh Infer vars from shared_unify)
3. Typeck CLOSURE MIR bodies FIRST (resolves return types, including
   Closure types for nested closures)
4. Update fn_sigs with resolved types from local_decls
5. Typeck MAIN body (uses resolved closure fn_sigs)

### 2.4 Additional Fixes (Discovered During Implementation)

1. **`AggregateKind::Closure` type inference**: Was returning a fresh Infer
   var, causing the closure literal's type to be lost. Fixed to return
   `Ty::new(TyKind::Closure(*def_id, substs.clone()), Span::DUMMY)`.

2. **`Closure` not accepted as callable**: `post_check_terminator` and
   `check_terminator` only accepted `FnDef` and `FnPtr`. Added
   `TyKind::Closure(_, _)` to the accepted list.

3. **`closure_def_id_counter` reset**: `new_with_unify` was resetting the
   counter to 0, causing DefId collisions between outer and nested
   closures. Fixed by passing the counter through.

4. **`param_count` hardcoded**: `codegen_synthesized_closure_functions`
   hardcoded `param_count = 2`. Fixed to read from `fn_sigs[def_id].inputs.len()`.

5. **Closure Copy derivation**: Closures with captures were always non-Copy.
   This caused borrowck false positives for `f()()` patterns (where `f`
   returns a closure with i32 captures). Fixed: closures with ALL-Copy
   captures are Copy (mirrors Rust's `#[derive(Copy)]`).

## 3. Architecture Changes

### 3.1 New API: `MirLowerCtxt::new_with_unify`

```rust
pub fn new_with_unify(
    interner: &'a Rodeo,
    span: Span,
    unify: UnificationTable,
    closure_def_id_counter: u32,
) -> Self
```

Constructs a `MirLowerCtxt` with an EXISTING `UnificationTable` and
`closure_def_id_counter`. Used by `build_synthesized_closure_mir_body` to
share the unify table and DefId space with the main body.

### 3.2 New API: `TypeChecker::into_results_with_unify`

```rust
pub fn into_results_with_unify(mut self)
    -> (Vec<TypeError>, TypeckResults, UnificationTable)
```

Consumes the type checker and returns the unify table back, so the caller
can pass it to the next `TypeChecker` (for chained typeck on main body +
closure MIR bodies sharing the same unify table).

### 3.3 Updated API: `build_synthesized_closure_mir_body`

```rust
pub fn build_synthesized_closure_mir_body(
    func: &SynthesizedClosureFunction,
    interner: &Rodeo,
    hir: &HirCrate,
    unify: UnificationTable,           // NEW: shared unify table
    closure_def_id_counter: u32,       // NEW: shared DefId counter
) -> (
    MirBody,
    UnificationTable,                  // returned (updated)
    Vec<crate::typeck::TypeError>,
    std::collections::HashMap<crate::hir::DefId, SynthesizedClosureFunction>,  // NEW: nested closures
    u32,                               // NEW: updated counter
)
```

### 3.4 Driver Flow (Updated)

```
For each body in hir.bodies:
  1. Lower HIR body → main_mir, shared_unify, synthesized_closures
  2. Build all closure MIR bodies (worklist, handles nested):
     - Pass shared_unify IN, get it back (updated)
     - Pass shared_closure_def_id_counter IN, get it back (updated)
     - Register fn_name + placeholder fn_sig
     - Collect nested closures for processing
  3. Typeck CLOSURE MIR bodies FIRST:
     - Use shared_unify (extract via into_results_with_unify)
     - Update fn_sig with resolved types from local_decls
     - Run drop elaboration
     - (Borrowck on closure MIR bodies deferred — TD-CLOSURE-BORROWCK-1)
  4. Typeck MAIN body:
     - Use shared_unify (now updated with closure body constraints)
     - Closure fn_sigs have resolved types → Call sites unify correctly
  5. Drop elaboration + borrowck on main body
```

### 3.5 Deprecated: `lower_closure_call_inline`

The inline closure call path (`lower_closure_call_inline`) is now
`#[deprecated]`. ALL closures use the synthesized `call` function path
(`lower_closure_call_to_synthesized`).

## 4. Test Coverage

### 4.1 Conformance Tests

- **Before**: 5222/5224 passing (2 failures: nested closure cases)
- **After**: 5224/5224 passing ✅

### 4.2 Integration Tests

- 2249/2249 passing ✅

### 4.3 Lib Tests

- 244/244 passing ✅

### 4.4 Runtime Verification

- `f(10) = 11` ✅ (no-capture closure)
- `x + y = 15` ✅ (i32 capture closure)
- Nested closure runtime: ❌ (codegen issue — TD-CLOSURE-CODEGEN-1)

## 5. Technical Debt

| ID | Description | Priority | Status |
|----|-------------|----------|--------|
| TD-CLOSURE-CODEGEN-1 | Codegen for calling `Closure`-typed local (nested closure runtime) | P2 | 🔧 Follow-up stage |
| TD-CLOSURE-BORROWCK-1 | Borrowck on closure MIR bodies (false positives on mutable captures in loops) | P2 | 🔧 Follow-up stage |
| TD-CLOSURE-2 | `closure_bodies` side-table duplicates `synthesized_closure_functions` | P3 | 🔧 Step 5 cleanup |
| TD-COPY-1 | `ty_is_copy` deprecated (test-only) | P3 | ✅ Documented |

## 6. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 244/244 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2249/2249 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 7717 tests passing, 0 failures, 0 warnings.**
- **Runtime**: `f(10) = 11` ✅, `x + y = 15` ✅

## 7. Version Policy

v0.229.6 → v0.230.0 (minor bump — 通解 typeck gap fix, ALL closures use
synthesized path, Closure Copy derivation, nested closure compile support.
Minor bump because the architecture change is significant: shared unify
table, new APIs, deprecated inline path.)

## 8. References

- Stage 16.28 analysis: `docs/develop/v0/stage-16/stage-16.28-complex-capture-analysis.md`
- Task 10 design: `docs/develop/v0/task-10-closure-redesign-design.md`
- v0.3 design: `docs/develop/v0/v0.3-complete-design.md`
- API naming standard: `docs/develop/v0/api-naming-standard.md`
- Stage committee process: `docs/stage-committee-process.md` §1.0 原則 6, 9
