# Stage 18.229 — v0.2.5e: Migrate `__landin_vec_push` → MIR Intrinsic

> **Date**: 2026-08-23
> **Version**: v0.477.0 → v0.478.0
> **Task ID**: stage18.229
> **Reviewer**: Super Z (main) — ARCH-A + DEV-A + REV-A + QA-A
> **流程文档**: docs/stage-committee-process.md v6.4 §13.1 + §12 + §17.6
> **设计文档**: docs/lang-design/06-mir.md §16.4 (MIR Intrinsic Ops migration plan)
> **任务审查**: docs/develop/v0/stage-18/stage-18.229-task-review.md

## 1. Scope

Per Stage 18.228 task-review §4.1: rewrite `lower_vec_push_intrinsic` to emit
MIR intrinsic ops (Load + GetElementPtr + Store + SwitchInt + Call(realloc))
instead of the `__landin_vec_push` C helper Call.

Per §17.6 (缺陷纳入): TD-C-WRAPPER-OVERUSE migration — 2nd of 4 C helpers.

## 2. Implementation

### 2.1 Migration Sequence (replaces C call)

8 basic blocks:
- bb0: Extract vec fields + need_grow check + SwitchInt
- grow_bb: is_zero check + SwitchInt
- zero_cap_bb: new_cap = 4
- nonzero_cap_bb: new_cap = cap * 2
- alloc_bb: realloc + update vec.ptr + vec.cap
- store_bb: reload vec.ptr + GEP + Store val + increment len

### 2.2 Critical Fixes (per §17.6 同类型整体修复)

| Fix | File | Description |
|-----|------|-------------|
| Borrowck StatementKind::Store | src/borrowck/mod.rs | `check_statement` now handles Store: calls `check_place_write` + `check_operand` |
| Borrowck PHI-like mutability | src/mir/lower/expr_variants.rs | `new_cap_local` created with `Mutability::Mutable` (assigned in 2 blocks) |
| Store Deref codegen | src/codegen/statement.rs | StatementKind::Store handles `Projection(base, Deref)` specially — loads POINTER from base, stores through it |
| push_statement API | src/mir/lower/mod.rs | New `push_statement(stmt, span)` method for arbitrary StatementKind |

### 2.3 Files Modified

| File | Change | LOC |
|------|--------|-----|
| `src/mir/lower/expr_variants.rs` | Rewrite `lower_vec_push_intrinsic` (~200 → ~370 LOC) | +170 |
| `src/mir/lower/mod.rs` | Add `push_statement` method | +12 |
| `src/borrowck/mod.rs` | Handle `StatementKind::Store` in `check_statement` | +15 |
| `src/codegen/statement.rs` | Handle `Projection(base, Deref)` in Store codegen | +20 |
| `docs/lang-design/06-mir.md` | Update §16.6 + §16.6.3 | +45 |

## 3. Test Verification (per §9.4)

### 3.1 Regression Tests (all pass)

| Test | Verifies |
|------|----------|
| `stage18_197_vec_push_single` | `v.push(1)` then `v.len()` returns 1 |
| `stage18_197_vec_push_multiple` | Multiple pushes work |
| `stage18_197_vec_push_growth` | Growth triggers (cap 4 → 8 after 5th push) |
| `stage18_197_vec_push_i64` | `Vec<i64>::push(42)` works |
| `stage18_197_vec_push_u8` | `Vec<u8>::push(255)` works |
| `stage18_197_vec_push_large_growth` | Large growth (5+ pushes) works |
| `stage18_203_vec_i32_roundtrip` | push + get roundtrip |
| `stage18_203_vec_i64_roundtrip` | i64 push + get roundtrip |
| `stage18_203_vec_i8_roundtrip` | i8 push + get roundtrip |
| `stage18_203_vec_u32_roundtrip` | u32 push + get roundtrip |
| `stage18_203_vec_i32_growth_roundtrip` | Growth + roundtrip |

## 4. 验收标准 (per §5.3)

| Criterion | Verification |
|-----------|-------------|
| `cargo build --release --features llvm-backend` | ✅ (45.19s) |
| `cargo check --features llvm-backend` | ✅ |
| `cargo fmt --check` | ✅ |
| `cargo clippy --all-targets --features llvm-backend -- -D warnings` | ✅ (0 warnings) |
| `cargo test --release --features llvm-backend` | ✅ (3783 tests, 0 failures) |

## 5. Design Principles Applied

- §1.0 原則 6 (通解>特例): one MIR sequence for all Vec<T> types
- §1.0 原則 4 (报错>静默): OOM panics via `__landin_realloc` (visible)
- §10 DRY: reuses `extract_vec_element_type`, `compute_type_size_with_fallback`, `MemoryEmitter` methods
- §11 接口隔离: MIR lowering emits MIR intrinsics; codegen only translates MIR
- §12 (最优 > 最小): typed Load + Store replaces byte-by-byte memcpy loop
- §17.6 (缺陷纳入): MVP scope (always realloc, no OOM check, PHI avoidance) recorded
