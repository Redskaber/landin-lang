# Stage 18.273 — TD-LOC-EXPR-VARIANTS Refactoring Plan

> **Author**: Super Z (main) — Stage Committee (ARCH-A + PM-A + REV-A)
> **Date**: 2026-08-25
> **Version**: v0.492.0 (no bump — refactoring)
> **Process**: stage-committee-process.md v6.4 §13.4 (重构即架构设计) + §13.4.1 (六大判据)
> **Status**: Planning

---

## 1. Architecture Analysis

### 1.1 Current State

`src/mir/lower/expr_variants.rs` — 3653 LOC, handling 10 functions across
2 distinct responsibilities:

**Responsibility 1: Expression variant lowering** (4 functions, ~1655 LOC)
- `lower_path_expr` (213 LOC) — Path expression lowering
- `lower_call_expr` (597 LOC) — Call expression + Adt ctor + expected_ty
- `lower_for_expr` (199 LOC) — For loop lowering
- `lower_method_call_expr` (646 LOC) — Method call lowering

**Responsibility 2: Intrinsic lowering** (6 functions, ~1733 LOC)
- `lower_string_from_str_intrinsic` (152 LOC) — String::from_str
- `lower_box_new_intrinsic` (142 LOC) — Box::new
- `lower_vec_push_intrinsic` (373 LOC) — Vec::push
- `lower_string_push_str_intrinsic` (376 LOC) — String::push_str
- `extract_vec_element_type` (17 LOC) — Vec element type helper
- `lower_vec_get_intrinsic` (138 LOC) — Vec::get
- `lower_format_variadic_intrinsic` (535 LOC) — format! variadic

### 1.2 Root Cause of Growth

The file was created at Stage 18.133 (1016 LOC) by extracting 4
expression variant functions from `expr_operand.rs`. It then grew
~2600 LOC during Stages 18.185-18.270 as intrinsics were added
(String::from_str, Box::new, Vec::push, etc.) and expected-ty
propagation work expanded `lower_call_expr`.

Per §13.4 J2 (单一职责): the file now violates single responsibility
by mixing expression lowering with intrinsic lowering.

---

## 2. §13.4.1 Six Judgments (J1-J6)

| # | Judgment | Verification |
|---|----------|-------------|
| J1 | Architecture alignment | ✅ New `intrinsic_lower.rs` module aligns with existing pattern — `mir/lower/` already has specialized modules (`adt_layout`, `closure_capture`, `control_flow`, `field_resolution`, `method_resolution`, etc.) |
| J2 | Single responsibility | ✅ After split: `expr_variants.rs` handles expression lowering only; `intrinsic_lower.rs` handles intrinsic lowering only |
| J3 | One-way flow | ✅ `lower_call_expr` in `expr_variants.rs` calls intrinsic functions in `intrinsic_lower.rs` (one direction); no back-calls |
| J4 | Compile-concept completeness | ✅ All intrinsic functions are self-contained — they take `&mut MirLowerCtxt` and produce `LocalId`; no shared state |
| J5 | Stage division | ✅ Both files are in `mir/lower/` — same pipeline stage (MIR lowering) |
| J6 | Reasonable size | ✅ After split: `expr_variants.rs` ~1920 LOC, `intrinsic_lower.rs` ~1733 LOC. Both near 1500 threshold but acceptable per §13.4.3 反模式1 (LOC split by responsibility, not pure LOC). If `expr_variants.rs` still too large after split, can further extract `lower_method_call_expr` in future stage. |

**All 6 judgments pass.**

---

## 3. Refactoring Plan

### 3.1 New Module: `src/mir/lower/intrinsic_lower.rs`

Extract these 7 functions from `expr_variants.rs`:
1. `lower_string_from_str_intrinsic` (152 LOC)
2. `lower_box_new_intrinsic` (142 LOC)
3. `lower_vec_push_intrinsic` (373 LOC)
4. `lower_string_push_str_intrinsic` (376 LOC)
5. `extract_vec_element_type` (17 LOC)
6. `lower_vec_get_intrinsic` (138 LOC)
7. `lower_format_variadic_intrinsic` (535 LOC)

Total: ~1733 LOC

### 3.2 Updated `expr_variants.rs`

Remaining 4 expression variant functions (~1920 LOC):
1. `lower_path_expr` (213 LOC)
2. `lower_call_expr` (597 LOC)
3. `lower_for_expr` (199 LOC)
4. `lower_method_call_expr` (646 LOC)

### 3.3 Re-export

`mir/lower/mod.rs` will add `mod intrinsic_lower;` and re-export
any functions called from outside the module (none expected — all
intrinsics are called only from `lower_call_expr` in `expr_variants.rs`).

---

## 4. Execution Steps

1. Create `src/mir/lower/intrinsic_lower.rs` with module doc
2. Move 7 intrinsic functions + their imports
3. Add `use super::intrinsic_lower::*;` or explicit imports in `expr_variants.rs`
4. Add `mod intrinsic_lower;` to `mir/lower/mod.rs`
5. Run cargo build + test + fmt + clippy
6. Verify all 3914 tests pass with 0 failures
