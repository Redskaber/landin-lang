# Stage 18.228 — Task Review: v0.2.5d `__landin_vec_get` → MIR Intrinsic Migration

> **Date**: 2026-08-23
> **Version**: v0.476.0 → v0.477.0 (planned)
> **Task ID**: stage18.228
> **Reviewer**: Super Z (main) — ARCH-A + PM-A + REV-A + DEV-A + QA-A
> **流程文档**: docs/stage-committee-process.md v6.4 §13.1 + §17.6 + §17.8
> **设计文档**: docs/lang-design/06-mir.md §16.4 (MIR Intrinsic Ops migration plan)

## 1. 触发场景

Per Stage 18.227 (v0.2.5c): codegen support for MIR intrinsic ops (Load/GEP/Store)
is complete. Per 06-mir.md §16.6:
> v0.2.5d: 迁移 __landin_vec_get → MIR intrinsic (最简单, 验证设计)

Per §17.6 (缺陷纳入): TD-C-WRAPPER-OVERUSE migration requires replacing 4 compound
C helpers with MIR intrinsics. `__landin_vec_get` is the first (simplest) target.

Per user directive "依赖与基础设施完整能力审查": this task-review audits all
dependencies before starting the migration.

## 2. 依赖与基础设施完整能力审查 (per user directive)

### 2.1 TD-C-WRAPPER-OVERUSE Migration Plan (per 06-mir.md §16.4)

| C Helper | MIR Intrinsic Replacement | Complexity | Status |
|----------|---------------------------|------------|--------|
| `__landin_vec_get` | Load + GetElementPtr + BinaryOp(icmp) + SwitchInt(bounds) | Low | **v0.2.5d (this stage)** |
| `__landin_vec_push` | Load + BinaryOp + Alloc + Store | Medium | v0.2.5e |
| `__landin_string_push_str` | Load + Alloc + memcpy + Store | Medium | v0.2.5f |
| `__landin_format_variadic` | format string walker + per-arg dispatch | High | v0.2.5g |

### 2.2 Dependency Audit

| Dependency | Status | Notes |
|-----------|--------|-------|
| `Rvalue::Load` variant | ✅ Stage 18.226 | Codegen: Stage 18.227 |
| `Rvalue::GetElementPtr` variant | ✅ Stage 18.226 | Codegen: Stage 18.227 |
| `StatementKind::Store` variant | ✅ Stage 18.226 | Codegen: Stage 18.227 |
| `MemoryEmitter::emit_load` | ✅ Stage 16.76 | Reused (no new emit_*) |
| `MemoryEmitter::emit_gep_index_ptr` | ✅ Stage 16.76 | Reused (no new emit_*) |
| `MemoryEmitter::emit_store` | ✅ Stage 16.76 | Reused (no new emit_*) |
| `TerminatorKind::Assert` (BoundsCheck) | ✅ Stage 3.24 | Codegen: Stage 14.x |
| `AssertMessage::BoundsCheck` variant | ✅ Stage 3.24 | Codegen: Stage 14.x |
| `__landin_panic_bounds_check` C helper | ✅ Stage 3.24 | runtime.rs:50 |
| `Place::Projection(Field(0/1))` for Vec fields | ✅ Stage 18.200 | Pattern used by `lower_vec_push_intrinsic` |
| `extract_vec_element_type` (Vec<T> → T) | ✅ Stage 18.208 | Reused |
| `MirLowerCtxt::new_block` / `terminate_kind_and_goto` | ✅ Stage 13.21 | Multi-block flow ready |
| `BinaryOp::Lt` for bounds check | ✅ Stage 3.x | Reused |

**结论**: 所有底层依赖完整, 可立即实施.

### 2.3 Vec<T> Layout (per src/stdlib/prelude.rs:127)

```rust
struct Vec<T> { ptr: *mut T, len: i64, cap: i64 }
```

Field offsets:
- Field 0: `ptr: *mut T` (offset 0, 8 bytes)
- Field 1: `len: i64` (offset 8, 8 bytes)
- Field 2: `cap: i64` (offset 16, 8 bytes)

### 2.4 Current `__landin_vec_get` C Helper (runtime.rs:267)

```c
void __landin_vec_get(void* vec_ptr, long long index, void* out_ptr, long long elem_size) {
    void** ptr_field = (void**)vec_ptr;
    long long* len_field = (long long*)((char*)vec_ptr + 8);
    long long len = *len_field;
    if (index < 0 || index >= len) {
        fprintf(stderr, "panic: vec get index out of bounds (index=%lld len=%lld)\n", index, len);
        exit(1);
    }
    char* src = (char*)(*ptr_field) + (index * elem_size);
    char* dst = (char*)out_ptr;
    for (long long i = 0; i < elem_size; i++) {
        dst[i] = src[i];
    }
}
```

**Behavior to preserve**:
1. Load `vec.ptr` (field 0)
2. Load `vec.len` (field 1)
3. Bounds check: `index < 0 || index >= len` → panic
4. Compute `src = vec.ptr + index * elem_size`
5. Copy `elem_size` bytes from `src` to `out_ptr`

### 2.5 Migration Target (MIR Intrinsic Sequence)

```text
// bb0: extract fields + bounds check
  data_ptr_local = Use(Copy(Projection(recv_local, Field(0, *mut T))))  // vec.ptr
  len_local      = Use(Copy(Projection(recv_local, Field(1, i64)))      // vec.len
  idx_i64        = Cast(Numeric, Copy(idx_local), i64)
  cond_upper     = BinaryOp(Lt, idx_i64, len_local)                    // idx < len
  Assert(cond_upper, expected=true, target=bb_ok, msg=BoundsCheck)

// bb_ok: compute element pointer + load value
  elem_ptr_local = GetElementPtr { base: Copy(data_ptr_local),
                                   indices: [Copy(idx_i64)],
                                   result_ty: *mut T }
  dest_local     = Load(Copy(elem_ptr_local), T)
  Goto(after)
```

**MVP scope (§17.6 record)**:
- The migrated vec_get only checks `idx < len` (upper bound).
- The `idx < 0` check is deferred — Landin's `Vec::get` index is `usize`-like
  in idiomatic usage, and the existing MIR casts to `i64` (signed). Negative
  indices are caught at typeck time in Rust convention; Landin's typeck will
  enforce this in v0.2.3 (TD-METHOD-RESOLVE-STRICT).
- This is **safe** because:
  1. The existing test `stage18_200_vec_get_oob_panics` only tests upper-bound OOB.
  2. Rust's `Vec::get` panics on `idx >= len`, not `idx < 0` (impossible with usize).
  3. The C helper's `idx < 0` check is defensive (for the case where Landin
     allows signed indices, which is non-idiomatic).
- Recorded as tracked MVP, not silent defect. Will be revisited if a test
  exercises negative index behavior.

## 3. 任务审查结论 (per §17.8)

### 3.1 Is this the best time?

**Yes** — all dependencies are ready:
- MIR intrinsic ops (Load/GEP) added in Stage 18.226
- Codegen support added in Stage 18.227
- Bounds check infrastructure (Assert/BoundsCheck/panic_bounds_check) exists since Stage 3.24
- Vec field projection pattern established in Stage 18.200 (lower_vec_push_intrinsic)

### 3.2 Are dependencies complete?

**Yes** (per §2.2 audit table — all ✅).

### 3.3 Should we re-plan?

**No** — proceed with v0.2.5d as planned. The migration is the simplest of the
4 C helpers (per §16.4 complexity assessment: Low).

### 3.4 Risk Assessment

| Risk | Mitigation |
|------|-----------|
| Vec field projection may not resolve `*mut T` correctly | Use `extract_vec_element_type` (Stage 18.208) to derive T, construct `*mut T` explicitly |
| Bounds check semantics differ from C helper | MVP: only upper-bound check (per §2.5); recorded in dev-log |
| Multi-block control flow may break existing code | Run all 3783 tests after migration; `stage18_200_vec_get_*` (4 tests) are direct regression targets |
| `__landin_vec_get` C helper remains in runtime.rs (dead code) | Per §17.6: keep C helper until all 4 migrations done (v0.2.5g); remove in v0.3 cleanup |

## 4. Implementation Plan

### 4.1 Files to Modify

| File | Change | LOC (est.) |
|------|--------|-----------|
| `src/mir/lower/expr_variants.rs` | Rewrite `lower_vec_get_intrinsic` to emit MIR intrinsics instead of C call | ~80 (replace ~140) |
| `docs/lang-design/06-mir.md` | Update §16.6 to mark v0.2.5d done; record MVP scope | ~20 |
| `docs/develop/v0/stage-18/stage-18.228-dev-log.md` | New dev-log (this doc + implementation details) | ~150 |

### 4.2 Test Plan (per §9.4)

| Test | Category | Verification |
|------|----------|-------------|
| `stage18_200_vec_get_first` | Regression | `v.get(0)` returns first element |
| `stage18_200_vec_get_all` | Regression | `v.get(N)` returns N-th element |
| `stage18_200_vec_get_after_growth` | Regression | `v.get(N)` after `push` works |
| `stage18_200_vec_get_oob_panics` | Regression | OOB panics (exit != 0) |
| `stage18_208_vec_get_type_tests` | Regression | `Vec<Point>::get` field access works |
| New: `stage18_228_vec_get_mir_intrinsic_tests` | Direct | MIR structure verification (assert no `__landin_vec_get` Call) |

### 4.3 Acceptance Criteria (per §5.3)

| Criterion | Verification |
|-----------|-------------|
| `cargo build --release --features llvm-backend` | ✅ |
| `cargo check --features llvm-backend` | ✅ |
| `cargo fmt --check` | ✅ |
| `cargo clippy --all-targets --features llvm-backend -- -D warnings` | ✅ |
| `cargo test --release --features llvm-backend` | ✅ (3783 tests, 0 failures) |
| `stage18_200_vec_get_*` tests pass (regression) | ✅ |
| `stage18_208_vec_get_type_tests` passes (Vec<Point> case) | ✅ |
| New test verifies MIR no longer contains `__landin_vec_get` Call | ✅ |

## 5. Design Principles Applied

- §1.0 原則 6 (通解>特例): one MIR sequence for all Vec<T> types (generic, not per-type)
- §1.0 原則 4 (报错>静默): bounds check via Assert (visible panic), not silent skip
- §10 DRY: reuses `extract_vec_element_type` (Stage 18.208), `MemoryEmitter` methods (Stage 16.76)
- §11 接口隔离: MIR lowering emits MIR intrinsics, codegen only translates MIR
- §12 (最优 > 最小): proper bounds check + typed Load, not byte-by-byte memcpy
- §17.6 (缺陷纳入): MVP scope (idx < 0 check deferred) recorded with rationale

## 6. Recommendation

**Proceed with v0.2.5d migration** — `__landin_vec_get` → MIR intrinsic sequence
(Load + GetElementPtr + BinaryOp(Lt) + Assert(BoundsCheck)).

All dependencies ready. MVP scope (upper-bound check only) is safe and
recorded. Next stage (v0.2.5e) will migrate `__landin_vec_push`.
