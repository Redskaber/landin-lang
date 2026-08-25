# Stage 18.230 — v0.2.5f: Migrate `__landin_string_push_str` → MIR Intrinsic

> **Date**: 2026-08-23
> **Version**: v0.478.0 → v0.479.0
> **Task ID**: stage18.230
> **Reviewer**: Super Z (main) — ARCH-A + DEV-A + REV-A + QA-A
> **流程文档**: docs/stage-committee-process.md v6.4 §13.1 + §12 + §17.6
> **设计文档**: docs/lang-design/06-mir.md §16.4 (MIR Intrinsic Ops migration plan)
> **任务审查**: docs/develop/v0/stage-18/stage-18.230-task-review.md

## 1. Scope

Per Stage 18.230 task-review: rewrite `lower_string_push_str_intrinsic` to emit
MIR intrinsic ops (Load + GetElementPtr + Store + SwitchInt + Call(realloc) +
Call(memcpy)) instead of the `__landin_string_push_str` C helper Call.

Per §17.6 (缺陷纳入): TD-C-WRAPPER-OVERUSE migration — 3rd of 4 C helpers.

## 2. Implementation

### 2.1 Migration Sequence (replaces C call)

10 basic blocks:
- bb0: Extract str fields + src fields; compute new_len; need_grow check
- grow_init_bb: is_zero check + SwitchInt
- zero_cap_bb: new_cap = 4
- nonzero_cap_bb: new_cap = cap
- grow_loop_bb: while (new_cap < new_len) — **BACK-EDGE TARGET**
- grow_body_bb: new_cap *= 2 — **BACK-EDGE**
- alloc_bb: realloc + Store str.ptr + Store str.cap
- copy_bb: reload str.ptr + GEP + Call memcpy + Store str.len

### 2.2 Key Differences from vec_push (Stage 18.229)

| Aspect | vec_push (18.229) | string_push_str (18.230) |
|--------|-------------------|--------------------------|
| Growth strategy | `new_cap = cap * 2` (single doubling) | `while (new_cap < new_len) new_cap *= 2` (while loop) |
| MIR structure | 8 basic blocks (straight-line + if/else) | 10 basic blocks (includes **MIR back-edge loop**) |
| Copy operation | Typed Store through `*elem_ptr = val` | `Call(__landin_memcpy, [dest, src, src_len])` |
| Element type | Generic T (from `extract_vec_element_type`) | Fixed u8 (String stores bytes) |

### 2.3 No New Bugs

All infrastructure fixes from Stages 18.228-18.229 applied directly:
- DCE handles Load/GEP/Store/Assert reads (Stage 18.228)
- Borrowck handles StatementKind::Store (Stage 18.229)
- Codegen handles Store Deref projection (Stage 18.229)
- `push_statement` API for arbitrary StatementKind (Stage 18.229)
- `new_local_with_mut` for PHI-like Mutable locals (Stage 18.229)

### 2.4 Files Modified

| File | Change | LOC |
|------|--------|-----|
| `src/mir/lower/expr_variants.rs` | Rewrite `lower_string_push_str_intrinsic` (~110 → ~370 LOC) | +260 |
| `docs/lang-design/06-mir.md` | Update §16.6 + §16.6.4 | +40 |

## 3. Test Verification (per §9.4)

### 3.1 Regression Tests (all pass)

| Test | Verifies |
|------|----------|
| `stage18_198_push_str_append` | `s.push_str(" world")` on "hello" → len=11 |
| `stage18_198_push_str_from_empty` | `s.push_str("hello")` on empty → len=5 |
| `stage18_198_push_str_multiple` | Multiple pushes → len=13 |
| `stage18_198_push_str_growth` | Growth triggers (cap=16 after 3 pushes — while loop correct) |
| `stage18_198_push_str_empty_src` | `s.push_str("")` → len unchanged |
| `stage18_198_push_str_long` | Long string (43 bytes) → len=43 |

## 4. 验收标准 (per §5.3)

| Criterion | Verification |
|-----------|-------------|
| `cargo build --release --features llvm-backend` | ✅ (44.77s) |
| `cargo check --features llvm-backend` | ✅ |
| `cargo fmt --check` | ✅ |
| `cargo clippy --all-targets --features llvm-backend -- -D warnings` | ✅ (0 warnings) |
| `cargo test --release --features llvm-backend` | ✅ (3783 tests, 0 failures) |

## 5. Design Principles Applied

- §1.0 原則 6 (通解>特例): one MIR sequence for all String::push_str calls
- §1.0 原則 4 (报错>静默): OOM panics via `__landin_realloc` (visible)
- §10 DRY: reuses `__landin_realloc` + `__landin_memcpy` (primitive helpers), `push_statement` API
- §11 接口隔离: MIR lowering emits MIR intrinsics; codegen only translates MIR
- §12 (最优 > 最小): typed Load + Store + memcpy replaces byte-by-byte C loop
- §17.6 (缺陷纳入): MVP scope (always realloc, no OOM check, PHI avoidance, memcpy via C) recorded
