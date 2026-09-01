# Stage 33.1 — Recursive collect_param_bindings + Vec::push/get Prelude Migration

> **Author**: PM-A + ARCH-A + DEV-A (Super Z)
> **Date**: 2026-09-01
> **Version**: v0.571.0 (target)
> **Stage**: v0.21 Stage 33.1
> **Predecessor**: v0.570.0 (v0.20 COMPLETE)
> **Tech-Debt Target**: TD-VEC-PUSH-GET-MIGRATION (P2) + TD-FORMAT-MIGRATION (P2, unblocked as side effect)

## §13.1 Design Alignment

Per §13.1 + §8.4.5: scanned `docs/lang-design/06-mir.md` (MonoItem design),
`docs/develop/v0/v0.5-roadmap.md` (Task 11 monomorphization ✅ Stage 16.49-16.62),
and the existing code:
- `src/mir/monomorphize/item.rs` — MonoItem collection (813 LOC, working)
- `src/mir/substitute.rs` — `substitute_mir_body` (working, substitutes Param in body)
- `src/codegen/function.rs:47` — `codegen_mono_functions` (working, emits specialized fns)
- `src/mir/lower/writeback.rs:918` — `collect_param_bindings` (**NON-RECURSIVE — root cause**)

## §1.2.1 Task Classification

L3 (cross-module: writeback.rs + prelude.rs + method_call_lower.rs + vec_intrinsics.rs deletion + tests). Full L3 process applies.

## 5W2H — Root Cause Analysis (Revised)

### WHAT
Stage 32.4's Vec::push/get migration failed because `collect_param_bindings`
(writeback.rs:918) is **non-recursive** — it only matches top-level `Param(N)`
against concrete types, not nested `Adt(def_id, [Param(N)])` against
`Adt(def_id, [concrete])`.

### WHY (root cause)
For `let v: Vec<i32> = Vec::new();`:
- `Vec::new()` has sig `fn new() -> Vec<T>` where T is the impl's generic param.
- `writeback_fndef_substs` tries to infer T from the destination local's type.
- It calls `collect_param_bindings(sig.output=Vec<T>, dest_ty=Vec<i32>)`.
- Current impl: `if param_ty is Param(N) → bindings[N] = concrete_ty`. But
  `Vec<T>` is `Adt(vec_def_id, [Param(0)])`, NOT `Param(0)`. So no binding
  is recorded. T stays as `Error` (fallback).
- Result: `MonoItem::Fn { def_id: vec_new_def_id, substs: [] }` (empty substs)
  → skipped by `codegen_mono_functions` (line 79: `if substs.is_empty() continue`).
- The body's `Param(0)` is never substituted → codegen falls back to i32.

### WHO
PM-A + ARCH-A + DEV-A.

### WHEN
Stage 33.1, immediately after Stage 32.5 (v0.20 complete).

### WHERE
- `src/mir/lower/writeback.rs:918` — fix `collect_param_bindings` to recurse.
- `src/stdlib/prelude.rs` — add Vec::push/get prelude impl bodies.
- `src/mir/lower/method_call_lower.rs` — remove Vec::push/get intrinsic dispatch.
- `src/mir/lower/vec_intrinsics.rs` — delete file.
- `src/mir/lower/mod.rs` — remove vec_intrinsics module.

### HOW
1. Make `collect_param_bindings` recursive — when both sides are the same
   variant (Adt vs Adt, Tuple vs Tuple, Ref vs Ref, etc.), recurse into
   their inner types.
2. Re-attempt Vec::push/get prelude migration (Stage 32.4 work, now unblocked).
3. Verify all existing Vec tests pass.
4. If successful, also unblock TD-FORMAT-MIGRATION (same root cause).

### HOW MUCH
- ~30 LOC writeback.rs fix (recursive collect_param_bindings)
- ~40 LOC prelude.rs additions (Vec::push/get bodies)
- ~647 LOC vec_intrinsics.rs deletion
- ~50 LOC method_call_lower.rs intrinsic dispatch removal
- ~10 new tests

## §12 Solution Choice

Per §12 (最优 > 最小): the OPTIMAL solution is the recursive
`collect_param_bindings` fix — it's a small, targeted fix that unblocks
TWO P2 TDs (TD-VEC-PUSH-GET-MIGRATION + TD-FORMAT-MIGRATION).

Per §1.0 原则 6 (通解 > 特解): one recursive function handles all nested
type matching, not per-type special cases.

Per §1.0 原则 9 (正确 > 妥协): root-cause fix (recursive binding), not
a hack (force Vec::new to be non-generic, etc.).

Per §1.0 原则 10 (唯一可信数据源): the dest local's type IS the source
of truth for inference — we just need to read it correctly (recursively).

## §14.8 Design Writeback (B1-B4)

### B1: Design vs. Implementation Match
- Recursive collect_param_bindings → enables correct T inference for Vec::new().
- Vec::push/get prelude impl → replaces 647 LOC intrinsic.

### B2: New TD Items
- None expected (this stage RESOLVES TD-VEC-PUSH-GET-MIGRATION).

### B3: Deviations
- None expected.

### B4: Architectural Limitations
- If recursive collect_param_bindings reveals MORE issues (e.g., projection
  types, higher-ranked bounds), they'll be documented as new TDs.

## Test Matrix (§9.4.3 — 1:3+ positive:negative)

### Positive Tests
- `Vec<i32>::push(42)` works (canonical case).
- `Vec<Point>::push(p)` works (struct element, the Stage 32.4 failure case).
- `Vec<i32>::get(0)` returns i32.
- `Vec<Point>::get(0).x` field access works.
- `Vec::new()` with type annotation infers T correctly.

### Negative Tests
- `Vec::push` on non-Vec type errors.
- `Vec::get` on non-Vec type errors.
- `Vec::push` with wrong arg type errors (e.g., `Vec<i32>::push(true)`).
- `Vec::get` with wrong index type errors.

## §1.6 终极检验

> "这是针对根因的最优架构解，还是仅仅为了跑通测试的最小补丁？"

**Answer**: This is the **root-cause architectural fix**. The recursive
`collect_param_bindings` is the correct general mechanism — it handles
ALL nested generic types (Vec<T>, Option<T>, Result<T,E>, Box<T>, etc.),
not just Vec. This unblocks TD-VEC-PUSH-GET-MIGRATION + TD-FORMAT-MIGRATION
+ potentially TD-SELF-OUTSIDE-IMPL-CONTEXT (if it depends on the same
inference path).
