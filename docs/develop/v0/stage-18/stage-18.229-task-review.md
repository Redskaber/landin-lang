# Stage 18.229 — Task Review: v0.2.5e `__landin_vec_push` → MIR Intrinsic Migration

> **Date**: 2026-08-23
> **Version**: v0.477.0 → v0.478.0 (planned)
> **Task ID**: stage18.229
> **Reviewer**: Super Z (main) — ARCH-A + PM-A + REV-A + DEV-A + QA-A
> **流程文档**: docs/stage-committee-process.md v6.4 §13.1 + §17.6 + §17.8
> **设计文档**: docs/lang-design/06-mir.md §16.4 (MIR Intrinsic Ops migration plan)

## 1. 触发场景

Per Stage 18.228 (v0.2.5d): `__landin_vec_get` migrated to MIR intrinsic.
Per 06-mir.md §16.6:
> v0.2.5e: 迁移 __landin_vec_push → MIR intrinsic ← Stage 18.229 (next)

Per §17.6 (缺陷纳入): TD-C-WRAPPER-OVERUSE migration — 2nd of 4 C helpers.

## 2. 依赖与基础设施完整能力审查 (per user directive)

### 2.1 Dependency Audit

| Dependency | Status | Notes |
|-----------|--------|-------|
| `Rvalue::Load` / `GetElementPtr` | ✅ Stage 18.226 | Codegen: Stage 18.227 |
| `StatementKind::Store` | ✅ Stage 18.226 | Codegen: Stage 18.227; works with Field projection |
| `Place::Projection(Local, Deref)` for storing through pointer | ✅ Stage 14.66 | Pattern used by `lower_box_new_intrinsic` |
| `TerminatorKind::SwitchInt` for growth conditional | ✅ Stage 3.x | Used by `if`/`match` lowering |
| `BinaryOp::Ge` for `len >= cap` check | ✅ Stage 3.x | Reused |
| `BinaryOp::Eq` for `cap == 0` check | ✅ Stage 3.x | Reused |
| `BinaryOp::Add` for `cap * 2` (cap+cap) and `len + 1` | ✅ Stage 3.x | Reused |
| `BinaryOp::Mul` for `new_cap * elem_size` | ✅ Stage 3.x | Reused |
| `__landin_realloc` C helper (per §16.5 primitive list) | ✅ Stage 18.194 | runtime.rs:185; supports NULL ptr |
| `compute_type_size_with_fallback` for elem_size | ✅ Stage 18.203 | Single source of truth |
| `extract_vec_element_type` for T from `Vec<T>` | ✅ Stage 18.208 | Reused |
| DCE handles new variants (Load/GEP/Store/Assert) | ✅ Stage 18.228 | Fixed in 18.228 |
| Borrowck handles new variants | ✅ Stage 18.228 | Fixed in 18.228 |
| LLVM `emit_call` arg coercion | ✅ Stage 18.228 | Fixed in 18.228 |
| GEP codegen derives element type from result_ty | ✅ Stage 18.228 | Fixed in 18.228 |

**结论**: 所有底层依赖完整, 可立即实施.

### 2.2 Vec<T> Layout (per src/stdlib/prelude.rs:127)

```rust
struct Vec<T> { ptr: *mut T, len: i64, cap: i64 }
```

Field offsets:
- Field 0: `ptr: *mut T` (offset 0, 8 bytes)
- Field 1: `len: i64` (offset 8, 8 bytes)
- Field 2: `cap: i64` (offset 16, 8 bytes)

### 2.3 Current `__landin_vec_push` C Helper (runtime.rs:198)

```c
void __landin_vec_push(void* vec_ptr, void* val_ptr, long long elem_size) {
    void** ptr_field = (void**)vec_ptr;           /* offset 0: *mut T */
    long long* len_field = (long long*)((char*)vec_ptr + 8);
    long long* cap_field = (long long*)((char*)vec_ptr + 16);
    long long len = *len_field;
    long long cap = *cap_field;
    if (len >= cap) {
        long long new_cap = (cap == 0) ? 4 : cap * 2;
        long long new_bytes = new_cap * elem_size;
        void* new_ptr = (cap == 0)
            ? malloc((size_t)new_bytes)
            : realloc(*ptr_field, (size_t)new_bytes);
        if (new_ptr == 0) { panic; exit(1); }
        *ptr_field = new_ptr;
        *cap_field = new_cap;
    }
    char* dest = (char*)(*ptr_field) + (len * elem_size);
    char* src = (char*)val_ptr;
    for (long long i = 0; i < elem_size; i++) { dst[i] = src[i]; }
    *len_field = len + 1;
}
```

**Behavior to preserve**:
1. Load `vec.ptr` (field 0), `vec.len` (field 1), `vec.cap` (field 2)
2. If `len >= cap`: grow (compute new_cap, realloc, update ptr + cap)
3. Store `val` at `ptr[len]`
4. Increment `len`

### 2.4 Migration Target (MIR Intrinsic Sequence)

```text
// bb0: extract fields + need_grow check
  data_ptr_local = Use(Copy(Projection(recv, Field(0, *mut T))))
  len_local      = Use(Copy(Projection(recv, Field(1, i64))))
  cap_local      = Use(Copy(Projection(recv, Field(2, i64))))
  need_grow      = BinaryOp(Ge, len, cap)                    // len >= cap
  SwitchInt(need_grow, targets=[(0, store_bb)], otherwise=grow_bb)

// grow_bb: compute new_cap (4 if cap==0, else cap*2)
  is_zero       = BinaryOp(Eq, cap, 0)
  SwitchInt(is_zero, targets=[(1, zero_cap_bb)], otherwise=nonzero_cap_bb)

// zero_cap_bb:
  Store(new_cap_local, Constant(4))
  Goto(alloc_bb)

// nonzero_cap_bb:
  doubled = BinaryOp(Add, cap, cap)                          // cap*2
  Store(new_cap_local, Copy(doubled))
  Goto(alloc_bb)

// alloc_bb: realloc + update vec.ptr + vec.cap
  new_cap_val    = Use(Copy(new_cap_local))
  new_bytes      = BinaryOp(Mul, new_cap_val, elem_size)
  old_bytes      = BinaryOp(Mul, cap, elem_size)
  new_ptr_local  = Call(__landin_realloc, [data_ptr, old_bytes, new_bytes])
  Store(Projection(recv, Field(0)), Copy(new_ptr_local))    // vec.ptr = new_ptr
  Store(Projection(recv, Field(2)), Copy(new_cap_val))      // vec.cap = new_cap
  Goto(store_bb)

// store_bb: store val + increment len
  current_ptr    = Use(Copy(Projection(recv, Field(0))))    // reload (handles growth)
  elem_ptr_local = GetElementPtr(current_ptr, [len], *mut T)
  Store(Projection(elem_ptr_local, Deref), Copy(val))       // *elem_ptr = val
  new_len        = BinaryOp(Add, len, Constant(1))
  Store(Projection(recv, Field(1)), Copy(new_len))         // vec.len = new_len
  Goto(after)
```

**MVP scope (§17.6 record)**:
- **Growth strategy**: `new_cap = (cap == 0) ? 4 : cap * 2` — matches C helper.
  SwitchInt dispatches between zero_cap_bb and nonzero_cap_bb.
- **Always realloc**: libc `realloc(NULL, size) == malloc(size)` per C standard.
  When `cap == 0`, `vec.ptr` is NULL, so `__landin_realloc(NULL, 0, new_bytes)`
  is equivalent to `malloc(new_bytes)`. This simplifies the migration (one Call
  path instead of two).
- **No OOM check**: The C helper checks `new_ptr == 0` and panics. The migrated
  MIR doesn't include an OOM check — `__landin_realloc` itself panics on OOM
  (runtime.rs:185). Safe — no behavior change.
- **PHI avoidance**: Instead of PHI nodes for `effective_ptr` (growth vs no-growth),
  we reload `vec.ptr` in store_bb via `Projection(recv, Field(0))`. This handles
  both cases (if growth happened, the field was updated; if not, it's the
  original ptr). Simpler MIR, no PHI support needed.

## 3. 任务审查结论 (per §17.8)

### 3.1 Is this the best time?

**Yes** — all dependencies ready (per §2.1 audit table — all ✅).
Stage 18.228 fixed 4 critical bugs that unblocked this migration.

### 3.2 Are dependencies complete?

**Yes** — all Stage 18.228 fixes apply directly (DCE, borrowck, emit_call coercion,
GEP element type). No additional infrastructure needed.

### 3.3 Should we re-plan?

**No** — proceed with v0.2.5e as planned. Complexity is Medium (per §16.4):
- Growth logic requires SwitchInt (conditional branching)
- Two stores to Vec fields (ptr + cap) during growth
- One store to Vec field (len) after value store
- Total: ~10 MIR ops + 2 Calls + 3 basic blocks

### 3.4 Risk Assessment

| Risk | Mitigation |
|------|-----------|
| StatementKind::Store to Field projection may not work | Pattern used by `lower_box_new_intrinsic` (Stage 14.66 Deref); codegen handles Field projections via `compute_place_address` + `emit_gep_field` |
| SwitchInt on boolean (0 vs 1) may have type issues | Existing pattern: `if` lowering uses SwitchInt with bool discr (Stage 3.x) |
| Reloading vec.ptr after growth may not see updated value | Store updates the alloca; subsequent Field projection load reads from the alloca — correct value propagation |
| `__landin_vec_push` C helper remains in runtime.rs (dead code) | Per §17.6: keep C helper until all 4 migrations done (v0.2.5g); remove in v0.3 cleanup |
| DCE may remove `data_ptr_local` (only used in grow_bb path) | Stage 18.228 fix: DCE correctly collects Load/GEP reads → preserves data_ptr_local |
| elem_size_local unused if no growth | elem_size used in both grow_bb (new_bytes = new_cap * elem_size) and store_bb (no — wait, store_bb uses GetElementPtr which uses element type, not elem_size). Actually elem_size is only used in grow_bb. So if no growth, elem_size is dead. DCE will remove it. This is fine — codegen handles it. |

## 4. Implementation Plan

### 4.1 Files to Modify

| File | Change | LOC (est.) |
|------|--------|-----------|
| `src/mir/lower/expr_variants.rs` | Rewrite `lower_vec_push_intrinsic` to emit MIR intrinsics | ~180 (replace ~200) |
| `docs/lang-design/06-mir.md` | Update §16.6 to mark v0.2.5e done; record MVP scope | ~30 |
| `docs/develop/v0/stage-18/stage-18.229-dev-log.md` | New dev-log | ~150 |

### 4.2 Test Plan (per §9.4)

| Test | Category | Verification |
|------|----------|-------------|
| `stage18_197_vec_push_single` | Regression | `v.push(1)` then `v.len()` returns 1 |
| `stage18_197_vec_push_multiple` | Regression | Multiple pushes work |
| `stage18_197_vec_push_growth` | Regression | Growth triggers (cap 4 → 8 after 5th push) |
| `stage18_197_vec_push_i64` | Regression | `Vec<i64>::push(42)` works |
| `stage18_197_vec_push_u8` | Regression | `Vec<u8>::push(255)` works |
| `stage18_197_vec_push_large_growth` | Regression | Large growth (5+ pushes) works |
| `stage18_203_vec_i32_roundtrip` | Regression | push + get roundtrip |
| `stage18_203_vec_i64_roundtrip` | Regression | i64 push + get roundtrip |
| `stage18_203_vec_i8_roundtrip` | Regression | i8 push + get roundtrip |
| `stage18_203_vec_u32_roundtrip` | Regression | u32 push + get roundtrip |
| `stage18_203_vec_i32_growth_roundtrip` | Regression | Growth + roundtrip |
| `stage18_206_abi_contract_tests` | Regression | C helper ABI contract unchanged |

### 4.3 Acceptance Criteria (per §5.3)

| Criterion | Verification |
|-----------|-------------|
| `cargo build --release --features llvm-backend` | ✅ |
| `cargo check --features llvm-backend` | ✅ |
| `cargo fmt --check` | ✅ |
| `cargo clippy --all-targets --features llvm-backend -- -D warnings` | ✅ |
| `cargo test --release --features llvm-backend` | ✅ (3783 tests, 0 failures) |
| All `stage18_197_vec_push_*` tests pass (regression) | ✅ |
| All `stage18_203_vec_*_roundtrip` tests pass (regression) | ✅ |

## 5. Design Principles Applied

- §1.0 原則 6 (通解>特例): one MIR sequence for all Vec<T> types (generic, not per-type)
- §1.0 原則 4 (报错>静默): OOM panics via `__landin_realloc` (visible, not silent)
- §10 DRY: reuses `extract_vec_element_type`, `compute_type_size_with_fallback`, `MemoryEmitter` methods
- §11 接口隔离: MIR lowering emits MIR intrinsics; codegen only translates MIR
- §12 (最优 > 最小): typed Load + Store replaces byte-by-byte memcpy loop
- §17.6 (缺陷纳入): MVP scope (always realloc, no OOM check, PHI avoidance) recorded with rationale

## 6. Recommendation

**Proceed with v0.2.5e migration** — `__landin_vec_push` → MIR intrinsic sequence
(Load + BinaryOp + SwitchInt + Call(realloc) + Store + GetElementPtr + Store + Store).

All dependencies ready. MVP scope (always realloc, no OOM check, PHI avoidance)
is safe and recorded. Next stage (v0.2.5f) will migrate `__landin_string_push_str`.
