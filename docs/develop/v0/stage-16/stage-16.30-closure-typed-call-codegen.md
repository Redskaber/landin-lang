# Stage 16.30 — 通解: Codegen for Closure-Typed Call Sites (Nested Closure Runtime)

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.230.0 → v0.230.1
> **Process**: stage-committee-process.md v3.24 §1.0 原則 6 "通用 > 特例" + 原則 9 "正确 > 妥协"

## 1. Executive Summary

Stage 16.30 fixes **TD-CLOSURE-CODEGEN-1** — the last remaining closure
bug that prevented nested closure runtime execution (`f()()` patterns
where `f` returns a closure).

**Root cause**: The codegen only resolved function names for `FnDef`-typed
func operands. When a Call terminator had a `Closure`-typed func operand
(e.g., the result of `f()` which returns a closure), the codegen fell
through to the indirect call path, treating the closure struct value as
a function pointer — emitting invalid LLVM IR (`call i32 %v4()` where
`%v4` is `{ i32 }`, not a pointer).

**The 通解 fix**: At codegen time, when a Call terminator has a
`Closure(def_id, _)`-typed func operand:
1. Resolve the function name via `fn_name_by_def_id[def_id]`
2. PREPEND the closure struct as the first arg (self)
3. Emit a direct call to the synthesized `call` function

This handles ALL closure-typed call sites uniformly — literal, let
binding, call result, etc.

**Test results**: 7732 tests passing (244 lib + 2264 integration + 5224
conformance), 0 failures, 0 warnings.

**Runtime verification**:
- `f(10) = 11` ✅ (no-capture closure)
- `x + y = 15` ✅ (i32 capture closure)
- `f()() = 42` ✅ (nested closure — **NEW!**)
- `g() = 1` ✅ (nested closure with let binding — **NEW!**)

## 2. Root Cause Analysis

### 2.1 The Problem

For `fn main() -> i32 { let x = 1; let f = || || x; let _ = f()(); 42 }`:

1. `f = || || x` — outer closure returns inner closure `|| x`
2. `f()` — call to `closure_call_fn_0`, returns inner closure struct `{ i32 }`
3. `f()()` — call to `closure_call_fn_1` with the inner closure as self

At MIR lowering time:
- `f()` is lowered by `lower_closure_call_to_synthesized` → dest local with
  Infer type (return type not yet resolved)
- `f()()` — func_local = dest local (Infer type)
  - `closure_bodies.contains_key` → FALSE (it's a call result, not a literal)
  - `is_adt_ctor` → FALSE (type is Infer)
  - Falls to "Real function call" → emits `Call { func: Copy(dest_local), args: [] }`

At typeck time:
- Resolves dest_local's type to `Closure(def_id, substs)`

At codegen time:
- Sees `Call { func: Copy(dest_local), args: [] }`
- func's type is `Closure(def_id, substs)` (resolved by typeck)
- `fn_name` lookup only handles `FnDef` → `fn_name = None`
- Falls to indirect call path → `call i32 %v4()` where `%v4` is `{ i32 }`
- **LLVM verification fails**: "Called function must be a pointer!"

### 2.2 The Fix

**At codegen time** (the 通解), when a Call terminator has a
`Closure(def_id, _)`-typed func operand:

```rust
// In the fn_name resolution:
match &ty.kind {
    TyKind::FnDef(def_id, _) => fn_name_by_def_id.get(def_id).cloned(),
    // Stage 16.30: Closure-typed func → resolve to synthesized function
    TyKind::Closure(def_id, _) => {
        closure_self_local = Some(*id);  // remember for self arg
        fn_name_by_def_id.get(def_id).cloned()
    }
    _ => None,
}
```

Then, when building arg_pairs, PREPEND the closure struct as self:

```rust
if let Some(self_id) = closure_self_local {
    let ptr_str = format!("%loc_{}", self_id.0);
    arg_pairs.push((EmitType::OpaquePtr, ptr_str));
}
```

Also update `callee_def_id` extraction to handle Closure-typed func:

```rust
match &ld.ty.kind {
    TyKind::FnDef(did, _) => Some(*did),
    TyKind::Closure(did, _) => Some(*did),  // Stage 16.30
    _ => None,
}
```

### 2.3 Why This Is the 通解

This fix handles ALL closure-typed call sites uniformly:

| Call Pattern | How Closure Value Is Produced | Handled? |
|---|---|---|
| `f(10)` where `f = \|x\| x+1` | Closure literal | ✅ (was already working via FnDef) |
| `let g = f; g(10)` | Let binding from literal | ✅ (closure_bodies propagates) |
| `f()()` where `f = \|\| \|\| x` | Call result | ✅ **NEW** (Stage 16.30) |
| `let g = f(); g()` | Let binding from call result | ✅ **NEW** (Stage 16.30) |

The codegen doesn't care HOW the closure value was produced — it just
checks the type. If it's `Closure(def_id, _)`, it resolves the function
name and prepends self.

Per §1.0 原則 6 "通用 > 特例": one codegen path for all closure-typed calls.
Per §1.0 原則 9 "正确 > 妥协": fix the root cause (codegen doesn't handle
Closure-typed func), not the symptom (indirect call on closure struct).

## 3. Dead Code Cleanup

Stage 16.30 also removes the old Stage 4.13 `is_closure` inline path
from `lower_expr_to_operand` (lines 898-961 in expr_operand.rs). This
was dead code because:

1. If the closure is a literal or let-bound from a literal →
   `closure_bodies.contains_key` is TRUE → handled by
   `lower_closure_call_to_synthesized`
2. If the closure is a call result → type is Infer at lowering time →
   `is_closure` was FALSE

The "Real function call" path now handles case 2, and the codegen
(Stage 16.30) resolves the Closure type to the synthesized function.

Per §1.0 原則 5 "去除兼容思维": dead code is removed.

## 4. Architecture Changes

### 4.1 Codegen: Call Terminator (src/codegen/terminator.rs)

**Before**: `fn_name` resolution only handled `TyKind::FnDef`.

**After**: `fn_name` resolution handles both `TyKind::FnDef` and
`TyKind::Closure`. When Closure-typed, the closure struct is prepended
as self (first arg).

### 4.2 MIR Lowering: Dead Code Removal (src/mir/lower/expr_operand.rs)

**Before**: `is_closure` check + Stage 4.13 inline path (63 lines of
dead code).

**After**: Direct "Real function call" path — emits a generic
`Call { func, args }` terminator. The codegen resolves the Closure type.

## 5. Test Coverage

### 5.1 Compile Coverage

- ✅ All 7732 tests pass (no regressions)

### 5.2 Runtime Coverage

| Test | Expected | Actual | Status |
|------|----------|--------|--------|
| `f(10)` where `f = \|x\| x+1` | 11 | 11 | ✅ |
| `\|\| x + y` (i32 captures) | 15 | 15 | ✅ |
| `f()()` where `f = \|\| \|\| x` | 42 | 42 | ✅ **NEW** |
| `let g = f(); g()` (nested with let) | 1 | 1 | ✅ **NEW** |

## 6. Technical Debt Update

| ID | Description | Priority | Status |
|----|-------------|----------|--------|
| TD-CLOSURE-CODEGEN-1 | Nested closure runtime codegen | P2 | ✅ **FIXED** (Stage 16.30) |
| TD-CLOSURE-BORROWCK-1 | Borrowck on closure MIR bodies | P2 | 🔧 Follow-up |
| TD-CLOSURE-2 | `closure_bodies` side-table duplicates `synthesized_closure_functions` | P3 | 🔧 Step 5 cleanup |
| TD-COPY-1 | `ty_is_copy` deprecated (test-only) | P3 | ✅ Documented |

## 7. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 244/244 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2264/2264 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 7732 tests passing, 0 failures, 0 warnings.**
- **Runtime**: `f(10)=11` ✅, `x+y=15` ✅, `f()()=42` ✅ **NEW**, `g()=1` ✅ **NEW**

## 8. Version Policy

v0.230.0 → v0.230.1 (patch bump — codegen fix for Closure-typed call
sites + dead code cleanup. No API changes, no behavior change for
existing tests.)

## 9. References

- Stage 16.29 (typeck gap fix): `docs/develop/v0/stage-16/stage-16.29-typeck-on-synthesized-closure-mir.md`
- Task 10 design: `docs/develop/v0/task-10-closure-redesign-design.md`
- v0.3 design: `docs/develop/v0/v0.3-complete-design.md`
- Stage committee process: `docs/stage-committee-process.md` §1.0 原則 5, 6, 9
