# Stage 18.236 — Task Review: Pointer Arithmetic Language Feature

> **Date**: 2026-08-23
> **Version**: v0.482.0 → v0.483.0 (planned)
> **Task ID**: stage18.236
> **Reviewer**: Super Z (main) — ARCH-A + PM-A + REV-A + DEV-A + QA-A
> **流程文档**: docs/stage-committee-process.md v6.4 §13.1 + §17.6 + §17.8
> **设计文档**: docs/lang-design/03-type-system.md + 06-mir.md §16.8

## 1. 触发场景

Per Stage 18.235: TD-INTRINSIC-OVERUSE deferred to v0.3, blocked by pointer
arithmetic language feature. Per user directive "依赖与基础设施完整能力审查":
pointer arithmetic is the prerequisite for the 通解 (stdlib impl migration).

Per §17.8 (任务审查): this is the blocking dependency. Implementing it now
unblocks the entire TD-INTRINSIC-OVERUSE migration chain.

## 2. 依赖与基础设施完整能力审查

### 2.1 Current State

```landin
let p: *mut u8 = __landin_alloc(16);
let q: *mut u8 = p + 1;  // FAILS: "cannot apply arithmetic to *mut u8"
```

- typeck `is_arithmetic_ty` only accepts Int/Uint/Float/Infer/Error — RawPtr is rejected
- MIR lower `lower_binary_op` generates `Rvalue::BinaryOp(Add, ptr, int)` — no pointer handling
- codegen `codegen_rvalue` BinaryOp Add path — would emit `LLVMBuildAdd` (integer add), not GEP

### 2.2 Dependencies

| Dependency | Status | Notes |
|-----------|--------|-------|
| `*mut T` / `*const T` type parsing | ✅ | Already supported |
| `extern "C"` block parsing + HIR lowering | ✅ | Already used for `__landin_alloc` etc. |
| `as *mut T` pointer cast | ✅ | `CastKind::Pointer` already exists |
| `CastKind::Pointer` codegen | ✅ | Already handled |
| typeck `is_arithmetic_ty` | ✅ (exists, needs update) | Add RawPtr support |
| MIR `Rvalue::BinaryOp` | ✅ (exists) | Add pointer+int lowering |
| codegen `BinaryOp::Add` | ✅ (exists) | Add pointer+int → GEP |
| `Rvalue::GetElementPtr` | ✅ Stage 18.226 | Can reuse for codegen |

**结论**: All infrastructure exists. The fix is:
1. typeck: Allow `ptr + int` (RawPtr is arithmetic-able for Add/Sub)
2. MIR lower: Lower `ptr + int` to `Rvalue::GetElementPtr` (reuse existing)
3. codegen: Already handles GetElementPtr (Stage 18.227)

No new language feature needed — just extend existing infrastructure.

### 2.3 Design Decision

**Approach**: When typeck sees `BinaryOp(Add, ptr, int)` or `BinaryOp(Add, int, ptr)`:
- If one operand is RawPtr and the other is integer → result type is the RawPtr type
- typeck accepts this (extend `is_arithmetic_ty` or add a special case in BinaryOp check)

When MIR lower sees `BinaryOp(Add, ptr_operand, int_operand)`:
- If the ptr operand's type is RawPtr → lower to `Rvalue::GetElementPtr { base: ptr, indices: [int] }`
- This reuses the existing GEP infrastructure (Stage 18.226-18.227)

When codegen sees `Rvalue::GetElementPtr`:
- Already handled (Stage 18.227) — emits `getelementptr inbounds`

**Per §1.0 原則 6 (通解 > 特解)**: Reuse `GetElementPtr` (existing 通解 for
pointer indexing) instead of creating a new pointer-arithmetic-specific Rvalue.

**Per §10 (DRY)**: No new MIR variant, no new codegen path — just wire
`ptr + int` to the existing GEP path.

## 3. Implementation Plan

### 3.1 Files to Modify

| File | Change | LOC (est.) |
|------|--------|-----------|
| `src/typeck/infer.rs` | BinaryOp Add: if one operand is RawPtr, result = RawPtr type | +20 |
| `src/mir/lower/expr_operand.rs` | lower_binary_op: if ptr+int, emit GetElementPtr | +30 |
| `src/typeck/check.rs` | post_check: accept ptr+int result (no mismatch) | +5 |
| `tests/v0/stage18/plan/stage18_236_ptr_arith_tests.rs` | New tests | ~80 |

### 3.2 Test Plan (per §9.4)

| Test | Category | Expected |
|------|----------|----------|
| `stage18_236_ptr_add_int` | Positive | `p + 1` compiles, result is *mut T |
| `stage18_236_ptr_add_zero` | Positive | `p + 0` is no-op |
| `stage18_236_ptr_sub_int` | Positive | `p - 1` compiles (backward offset) |
| `stage18_236_ptr_add_var` | Positive | `p + n` where n is a variable |
| `stage18_236_ptr_store_through_offset` | Positive | `*(p + 1) = 42` works |
| `stage18_236_ptr_add_ptr_fails` | Negative | `p + q` should fail (ptr + ptr not allowed) |
| `stage18_236_ptr_add_float_fails` | Negative | `p + 1.0` should fail |

## 4. Recommendation

**Proceed with pointer arithmetic** using the GetElementPtr reuse approach.

This is the prerequisite for TD-INTRINSIC-OVERUSE migration. The fix is
localized and reuses existing infrastructure (no new MIR variants, no new
codegen paths). Per §1.0 原則 6 (通解 > 特解), this is the 通解.
