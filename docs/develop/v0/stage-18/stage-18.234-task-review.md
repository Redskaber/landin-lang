# Stage 18.234 — Task Review: TD-METHOD-RESOLVE-STRICT Fix

> **Date**: 2026-08-23
> **Version**: v0.481.0 → v0.482.0 (planned)
> **Task ID**: stage18.234
> **Reviewer**: Super Z (main) — ARCH-A + PM-A + REV-A + DEV-A + QA-A
> **流程文档**: docs/stage-committee-process.md v6.4 §13.1 + §17.6 + §17.8

## 1. 触发场景

Per Stage 18.233: TD-TUPLE-CTOR-TYPECK deferred to v0.3 (requires architecture
changes). Per user directive "同类型错误或者存在依赖关系（情况）的应该考虑整体性
完整修复", proceed with TD-METHOD-RESOLVE-STRICT (v0.2.3) which is independent.

## 2. Root Cause Analysis

### 2.1 Bug Reproduction

```landin
fn main() {
    let s = String::new();   // s has Infer type at MIR lower time
    s.nonexistent_method();  // SHOULD fail: method not found
}
```

Currently compiles without error. When `s` has explicit type (`let s: String`),
the error IS reported. The difference: at MIR lower time, `String::new()` returns
an Infer type, so `s` is Infer. The method resolution is skipped (is_known_unsupported
check), and the Error placeholder is silently accepted by typeck.

### 2.2 Root Cause

In `lower_method_call_expr` (expr_variants.rs:1388):
```rust
let is_known_unsupported = matches!(
    &recv_ty.kind,
    TyKind::Error | TyKind::Ref(_, _, _) | TyKind::Infer(_)
);
if !is_known_unsupported {
    cx.type_errors.push("no method found");
}
```

For Infer receiver types, the error is NOT pushed (to allow typeck to resolve
the type first). But typeck never re-checks method resolution after defaulting.

In `post_check_terminator` (check.rs:93):
```rust
if !matches!(
    &func_ty.kind,
    TyKind::FnDef | TyKind::FnPtr | TyKind::Closure | TyKind::Error
) { ... }
```

When func is Error, it's accepted (listed in matches!). So the Error placeholder
from the unresolved method call passes typeck silently.

### 2.3 The Fix

**Approach**: Deferred method resolution tracking.

1. Add a `deferred_method_calls` side-table to `MirBody` that records:
   - The receiver local ID
   - The method name
   - The source span
2. In `lower_method_call_expr`, when `is_known_unsupported` is TRUE (Infer receiver)
   AND method resolution failed, record the call in the side-table.
3. In `post_check_terminator` (after typeck defaulting), for each deferred call:
   - Resolve the receiver local's type (now concrete)
   - Re-attempt method resolution
   - If still not found, report "no method found" error

This is the holistic fix per user directive — it tracks the method resolution
failure through typeck defaulting and reports it after the type is known.

## 3. 依赖与基础设施完整能力审查

| Dependency | Status | Notes |
|-----------|--------|-------|
| MirBody struct (extensible) | ✅ | Can add side-table field |
| `resolve_inherent_method` | ✅ Stage 14.91 | Reusable for re-check |
| `resolve_trait_method` | ✅ Stage 14.91 | Reusable for re-check |
| typeck `post_check_terminator` | ✅ Stage 18.72 | Runs after defaulting |
| `cx.hir` access in typeck | ✅ | Available via `TypeChecker` |

**结论**: All dependencies ready. The fix is localized — add side-table + re-check logic.

## 4. Implementation Plan

### 4.1 Files to Modify

| File | Change | LOC (est.) |
|------|--------|-----------|
| `src/mir/body.rs` | Add `deferred_method_calls: Vec<DeferredMethodCall>` field | +15 |
| `src/mir/lower/expr_variants.rs` | Record deferred calls when recv is Infer | +20 |
| `src/typeck/check.rs` | Re-check deferred calls in post_check_terminator | +30 |
| `tests/v0/stage18/plan/stage18_234_method_resolve_tests.rs` | New regression tests | ~80 |

### 4.2 Test Plan (per §9.4)

| Test | Category | Expected |
|------|----------|----------|
| `stage18_234_infer_recv_unknown_method` | Negative | `let s = String::new(); s.foo()` fails |
| `stage18_234_explicit_recv_unknown_method` | Regression | `let s: String = String::new(); s.foo()` fails (still) |
| `stage18_234_infer_recv_known_method` | Positive | `let s = String::new(); s.len()` works |
| `stage18_234_infer_recv_valid_method` | Positive | `let v = Vec::new(); v.push(1)` works |

## 5. Recommendation

**Proceed with TD-METHOD-RESOLVE-STRICT fix** using deferred method resolution tracking.

This is a type safety issue — silently accepting calls to nonexistent methods
violates §1.0 原則 4 (报错>静默). The fix is localized and doesn't require
lowering architecture changes.
