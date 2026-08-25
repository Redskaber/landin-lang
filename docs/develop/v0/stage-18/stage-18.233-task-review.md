# Stage 18.233 — TD-TUPLE-CTOR-TYPECK Audit (Partial Fix, Deferred)

> **Date**: 2026-08-23
> **Version**: v0.481.0 (no bump — audit + documentation)
> **Task ID**: stage18.233
> **Reviewer**: Super Z (main) — ARCH-A + PM-A + REV-A + DEV-A + QA-A
> **流程文档**: docs/stage-committee-process.md v6.4 §13.1 + §17.6 + §17.8
> **设计文档**: docs/lang-design/03-type-system.md

## 1. 触发场景

Per Stage 18.232 deep review: v0.2 Phase 2 complete, next TDs include
TD-TUPLE-CTOR-TYPECK (v0.2 P2+). Per user directive "同类型错误或者存在
依赖关系（情况）的应该考虑整体性完整修复".

## 2. Root Cause Analysis (Deep)

### 2.1 Bug Reproduction

```landin
struct Wrapper<T>(T);
fn main() {
    let w: Wrapper<i32> = Wrapper(true);  // SHOULD fail: bool ≠ i32
}
```

Currently compiles without error.

### 2.2 Root Cause Chain

1. `Wrapper(true)` is parsed as `Call { func: Path("Wrapper"), args: [true] }`
2. `lower_path_expr(Wrapper)` creates a local with type `Adt(def_id, substs=[])`
   — substs is EMPTY because the path `Wrapper` has no `<i32>` annotation.
3. In `lower_call_expr`, `is_adt_ctor` is TRUE (func type is Adt).
4. `adt_substs` is `[]` (empty, from func_local_decl.ty).
5. `resolve_adt_field_tys(cx, adt_def_id)` is called (NOT `_with_substs`)
   because `adt_substs.is_empty()`. Returns `[Param(T)]`.
6. Aggregate is created: `Aggregate(Adt(def_id, [], [Param(T)]), [Move(bool_temp)])`.
7. This Aggregate is assigned to a **temp local** (not `w` directly).
8. typeck `check_statement` for `temp = Aggregate(...)`:
   - `place_ty = temp.ty = Adt(def_id, [])` (empty substs)
   - `infer_rvalue` unifies `bool` with `Param(T)` → Param unifies with anything → no error.
9. Then `w = Use(temp)`:
   - `place_ty = w.ty = Adt(def_id, [i32])` (from `let w: Wrapper<i32>` annotation)
   - `rvalue_ty = Adt(def_id, [])` (temp's type)
   - Adt unify: `if a_substs.is_empty() || b_substs.is_empty() { return Ok(()); }`
   - temp's substs is empty → unify succeeds silently.

### 2.3 Why Simple Fixes Fail

**Attempt 1 (infer_rvalue substitution)**: Pass `expected_dest_ty` to `infer_rvalue`.
When field_tys contain Param, substitute using expected_dest_ty's substs.
- **Problem**: The Aggregate is assigned to a **temp local** (with empty substs),
  not `w` (with `[i32]` substs). So `expected_dest_ty` is `Adt(def_id, [])`,
  and no substitution happens.

**Attempt 2 (Infer vars for generic params)**: In `lower_path_expr`, when the
path has no explicit generic args but the struct has generic params, create fresh
Infer vars as substs.
- **Problem**: The Infer var gets bound to the operand's type (`bool`), not the
  expected type (`i32`). Then `w = Use(temp)` unifies `[i32]` with `[Infer→bool]`,
  producing a spurious "mismatch" error even for correct code like `Wrapper<i32>(42)`.

### 2.4 The Real Fix (Deferred to v0.3)

The root cause is a **MIR lowering architecture issue**: tuple struct ctor
calls create a temp local, losing the expected type context. The fix requires
**expected type propagation** through the lowering pipeline:

1. In `lower_let_stmt`, when the `let` binding has a type annotation (`let w: Wrapper<i32>`),
   pass the annotation's substs to the init expression lowering.
2. In `lower_call_expr`, when `adt_substs` is empty AND an expected type is provided,
   use the expected type's substs.
3. This requires threading `expected_ty: Option<&Ty>` through all `lower_expr_*` functions.

This is a significant architectural change that touches the entire MIR lowering
pipeline. Per §17.6 (缺陷纳入), this is recorded as a deferred MVP with a clear plan.

## 3. Decision: DEFER (per §17.8 task review)

**Per user directive "如果当前设计和实现存在简写和缺陷或MVP（时机： 此条例触发时机）,
则需要将简写和缺陷的原因及描述等必要信息记录在开发、设计文档中并规划修订完整计划"**:

TD-TUPLE-CTOR-TYPECK is **deferred to v0.3** because:
1. The fix requires MIR lowering architecture changes (expected type propagation)
2. The current behavior is "silently accepts" (not a crash or miscompile for valid code)
3. The bug only manifests for **wrong code** (type mismatches) — valid code works correctly
4. v0.3 will have a proper expected-type infrastructure (needed for trait solver + GATs)

## 4. Revised Plan

| Task | Target | Reason |
|------|--------|--------|
| TD-TUPLE-CTOR-TYPECK | v0.3 | Requires expected-type propagation in MIR lower |
| TD-METHOD-RESOLVE-STRICT | v0.2.3 | Independent of this issue |
| TD-DROP-MOVED-LOCALS | v0.3+ | Already deferred |
| TD-BOX-AUTO-DROP | v0.3+ | Blocked by TD-DROP-MOVED-LOCALS |

## 5. Documentation Updates

- `docs/develop/v0/tech-debt-register.md`: Update TD-TUPLE-CTOR-TYPECK entry
  with root cause analysis + deferral rationale.
- `docs/lang-design/03-type-system.md`: (future) Add expected-type propagation
  design for v0.3.

## 6. Conclusion

**NO CODE CHANGES** — this is an audit stage. The TD is documented with full
root cause analysis and a clear v0.3 plan. Per §17.8, this is the correct
decision because the fix requires architectural changes that exceed the current
stage's scope.

**Next**: Proceed to TD-METHOD-RESOLVE-STRICT (v0.2.3) which is independent
and doesn't require lowering changes.
