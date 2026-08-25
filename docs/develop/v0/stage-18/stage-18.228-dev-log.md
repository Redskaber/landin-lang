# Stage 18.228 — v0.2.5d: Migrate `__landin_vec_get` → MIR Intrinsic

> **Date**: 2026-08-23
> **Version**: v0.476.0 → v0.477.0
> **Task ID**: stage18.228
> **Reviewer**: Super Z (main) — ARCH-A + DEV-A + REV-A + QA-A
> **流程文档**: docs/stage-committee-process.md v6.4 §13.1 + §12 + §17.6
> **设计文档**: docs/lang-design/06-mir.md §16.4 (MIR Intrinsic Ops migration plan)
> **任务审查**: docs/develop/v0/stage-18/stage-18.228-task-review.md

## 1. Scope

Per Stage 18.228 task-review (§4.1): rewrite `lower_vec_get_intrinsic` to emit
MIR intrinsic ops (Load + GetElementPtr + BinaryOp(Lt) + Assert(BoundsCheck))
instead of the `__landin_vec_get` C helper Call.

Per §17.6 (缺陷纳入): TD-C-WRAPPER-OVERUSE migration — first of 4 C helpers.

## 2. Implementation

### 2.1 Migration Sequence (replaces C call)

```text
// bb0: extract Vec fields + cast index + bounds check
  data_ptr_local = Use(Copy(Projection(recv_local, Field(0, *mut T))))  // vec.ptr
  len_local      = Use(Copy(Projection(recv_local, Field(1, i64)))       // vec.len
  idx_i64        = Cast(Numeric, Copy(idx_local), i64)
  cond_upper     = BinaryOp(Lt, Copy(idx_i64), Copy(len_local))         // idx < len
  Assert(cond_upper, expected=true, target=bb_ok, msg=BoundsCheck)

// bb_ok: compute element pointer + load value
  elem_ptr_local = GetElementPtr { base: Copy(data_ptr_local),
                                   indices: [Copy(idx_i64)],
                                   result_ty: *mut T }
  dest_local     = Load(Copy(elem_ptr_local), T)
  Goto(after)
```

### 2.2 Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| Use `Place::Projection(Field(0/1))` for Vec field access | Established pattern (Stage 18.200 `lower_vec_push_intrinsic`); reuses AdtLayout system |
| Use `extract_vec_element_type` (Stage 18.208) to derive T | Single source of truth; handles `&Vec<T>` unwrap |
| Use `TerminatorKind::Assert(BoundsCheck)` for panic | Existing infra (Stage 3.24); codegen emits `__landin_panic_bounds_check` |
| Use `Rvalue::GetElementPtr` (Stage 18.226) for element pointer | New MIR intrinsic; codegen via `emit_gep_index_ptr` (Stage 18.227) |
| Use `Rvalue::Load` (Stage 18.226) for element load | New MIR intrinsic; codegen via `emit_load` (Stage 18.227) |
| **MVP**: only check `idx < len` (upper bound) | Rust convention: usize indices can't be negative; recorded in task-review §2.5 |

### 2.3 Files Modified

| File | Change | LOC |
|------|--------|-----|
| `src/mir/lower/expr_variants.rs` | Rewrite `lower_vec_get_intrinsic` (~140 → ~90 LOC) | -50 |
| `docs/lang-design/06-mir.md` | Update §16.6 to mark v0.2.5d done | +5 |
| `docs/develop/v0/stage-18/stage-18.228-dev-log.md` | This doc | +150 |

## 3. Test Verification (per §9.4)

### 3.1 Regression Tests (must pass)

| Test | Verifies |
|------|----------|
| `stage18_200_vec_get_first` | `v.get(0)` returns first element |
| `stage18_200_vec_get_all` | `v.get(N)` returns N-th element |
| `stage18_200_vec_get_after_growth` | `v.get(N)` after `push` works |
| `stage18_200_vec_get_oob_panics` | OOB panics (exit != 0) |
| `stage18_208_vec_get_type_tests` | `Vec<Point>::get` field access works |

### 3.2 New Direct Test

| Test | Verifies |
|------|----------|
| `stage18_228_vec_get_mir_intrinsic_tests` | MIR no longer contains `__landin_vec_get` Call; uses Load/GEP/Assert instead |

## 4. 验收标准 (per §5.3)

| Criterion | Verification |
|-----------|-------------|
| `cargo build --release --features llvm-backend` | ✅ |
| `cargo check --features llvm-backend` | ✅ |
| `cargo fmt --check` | ✅ |
| `cargo clippy --all-targets --features llvm-backend -- -D warnings` | ✅ |
| `cargo test --release --features llvm-backend` | ✅ (3783 tests, 0 failures) |
| `stage18_200_vec_get_*` regression tests pass | ✅ |
| `stage18_208_vec_get_type_tests` passes | ✅ |

## 5. Design Principles Applied

- §1.0 原則 6 (通解>特例): one MIR sequence for all Vec<T> types
- §1.0 原則 4 (报错>静默): bounds check via Assert (visible panic)
- §10 DRY: reuses `extract_vec_element_type` (Stage 18.208), `MemoryEmitter` methods (Stage 16.76)
- §11 接口隔离: MIR lowering emits MIR intrinsics, codegen only translates MIR
- §12 (最优 > 最小): proper bounds check + typed Load, not byte-by-byte memcpy
- §17.6 (缺陷纳入): MVP scope (idx < 0 check deferred) recorded with rationale
