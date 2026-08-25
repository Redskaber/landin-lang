# Stage 18.230 — Task Review: v0.2.5f `__landin_string_push_str` → MIR Intrinsic Migration

> **Date**: 2026-08-23
> **Version**: v0.478.0 → v0.479.0 (planned)
> **Task ID**: stage18.230
> **Reviewer**: Super Z (main) — ARCH-A + PM-A + REV-A + DEV-A + QA-A
> **流程文档**: docs/stage-committee-process.md v6.4 §13.1 + §17.6 + §17.8
> **设计文档**: docs/lang-design/06-mir.md §16.4 (MIR Intrinsic Ops migration plan)

## 1. 触发场景

Per Stage 18.229 (v0.2.5e): `__landin_vec_push` migrated to MIR intrinsic.
Per 06-mir.md §16.6:
> v0.2.5f: 迁移 __landin_string_push_str → MIR intrinsic ← Stage 18.230 (next)

Per §17.6 (缺陷纳入): TD-C-WRAPPER-OVERUSE migration — 3rd of 4 C helpers.

## 2. 依赖与基础设施完整能力审查 (per user directive)

### 2.1 Dependency Audit

| Dependency | Status | Notes |
|-----------|--------|-------|
| `Rvalue::Load` / `GetElementPtr` | ✅ Stage 18.226 | Codegen: Stage 18.227 |
| `StatementKind::Store` + Deref codegen | ✅ Stage 18.229 | Fixed in 18.229 (borrowck + codegen) |
| `TerminatorKind::SwitchInt` for growth conditional | ✅ Stage 3.x | Used by vec_push migration (Stage 18.229) |
| `BinaryOp::Gt` for `new_len > cap` check | ✅ Stage 3.x | Reused |
| `BinaryOp::Lt` for while loop `new_cap < new_len` | ✅ Stage 3.x | Reused |
| `BinaryOp::Add` for `len + src_len` and `new_cap + new_cap` | ✅ Stage 3.x | Reused |
| `__landin_realloc` C helper | ✅ Stage 18.194 | runtime.rs:185; supports NULL ptr |
| `__landin_memcpy` C helper (per §16.5 primitive list) | ✅ Stage 18.185 | runtime.rs:173; DefId u32::MAX - 101 |
| `push_statement` API | ✅ Stage 18.229 | Added in 18.229 |
| `new_local_with_mut` for PHI-like Mutable locals | ✅ Stage 18.229 | Pattern from 18.229 |
| DCE handles Load/GEP/Store/Assert reads | ✅ Stage 18.228 | Fixed in 18.228 |
| Borrowck handles Store | ✅ Stage 18.229 | Fixed in 18.229 |
| LLVM `emit_call` arg coercion | ✅ Stage 18.228 | Fixed in 18.228 |
| GEP codegen derives element type from result_ty | ✅ Stage 18.228 | Fixed in 18.228 |
| MIR loop (back-edge) codegen | ✅ Stage 3.x | `while`/`for` lowering already uses back-edges |

**结论**: 所有底层依赖完整, 可立即实施.

### 2.2 String Layout (per src/stdlib/prelude.rs:103)

```rust
struct String { ptr: *mut u8, len: i64, cap: i64 }
```

Field offsets (same as Vec<T>):
- Field 0: `ptr: *mut u8` (offset 0, 8 bytes)
- Field 1: `len: i64` (offset 8, 8 bytes)
- Field 2: `cap: i64` (offset 16, 8 bytes)

### 2.3 Current `__landin_string_push_str` C Helper (runtime.rs:232)

```c
void __landin_string_push_str(void* str_ptr, const char* src_ptr, long long src_len) {
    void** ptr_field = (void**)str_ptr;           /* offset 0: *mut u8 */
    long long* len_field = (long long*)((char*)str_ptr + 8);
    long long* cap_field = (long long*)((char*)str_ptr + 16);
    long long len = *len_field;
    long long cap = *cap_field;
    long long new_len = len + src_len;
    if (new_len > cap) {
        long long new_cap = (cap == 0) ? 4 : cap;
        while (new_cap < new_len) new_cap *= 2;
        long long new_bytes = new_cap;
        void* new_ptr = (cap == 0)
            ? malloc((size_t)new_bytes)
            : realloc(*ptr_field, (size_t)new_bytes);
        if (new_ptr == 0) { panic; exit(1); }
        *ptr_field = new_ptr;
        *cap_field = new_cap;
    }
    char* dest = (char*)(*ptr_field) + len;
    for (long long i = 0; i < src_len; i++) { dest[i] = src_ptr[i]; }
    *len_field = new_len;
}
```

**Behavior to preserve**:
1. Load `str.ptr` (field 0), `str.len` (field 1), `str.cap` (field 2)
2. Extract `src.ptr` (field 0) and `src.len` (field 1) from `&str` fat pointer
3. Compute `new_len = len + src_len`
4. If `new_len > cap`: grow (while loop: new_cap = cap==0 ? 4 : cap; while new_cap < new_len: new_cap *= 2)
5. Copy `src_len` bytes from `src.ptr` to `str.ptr[len]`
6. Update `str.len = new_len`

### 2.4 Migration Target (MIR Intrinsic Sequence)

**Key difference from vec_push**: Growth strategy uses a **while loop** (not just `cap * 2`).
The while loop is expressed via a back-edge in MIR (grow_loop_bb → grow_body_bb → grow_loop_bb).

```
bb0: extract fields + compute new_len + need_grow check
  data_ptr = Field(recv, 0, *mut u8)
  len      = Field(recv, 1, i64)
  cap      = Field(recv, 2, i64)
  src_ptr  = Field(src, 0, *mut u8)
  src_len  = Field(src, 1, i64)
  new_len  = BinaryOp(Add, len, src_len)
  need_grow = BinaryOp(Gt, new_len, cap)
  SwitchInt(need_grow, [(0, copy_bb)], otherwise=grow_init_bb)

grow_init_bb: initialize new_cap based on cap==0
  is_zero = BinaryOp(Eq, cap, 0)
  SwitchInt(is_zero, [(1, zero_cap_bb)], otherwise=nonzero_cap_bb)

zero_cap_bb: new_cap = 4 (initial capacity)
  Store(new_cap_local, Constant(4))
  Goto(grow_loop_bb)

nonzero_cap_bb: new_cap = cap
  Store(new_cap_local, Copy(cap))
  Goto(grow_loop_bb)

grow_loop_bb: while (new_cap < new_len) new_cap *= 2  ← BACK-EDGE TARGET
  cond = BinaryOp(Lt, new_cap, new_len)
  SwitchInt(cond, [(0, alloc_bb)], otherwise=grow_body_bb)

grow_body_bb: new_cap = new_cap + new_cap (2x)
  doubled = BinaryOp(Add, new_cap, new_cap)
  Store(new_cap_local, Copy(doubled))
  Goto(grow_loop_bb)  ← BACK-EDGE

alloc_bb: realloc + update str.ptr + str.cap
  new_bytes = new_cap (String stores bytes, elem_size = 1)
  old_bytes = cap
  new_ptr = Call(__landin_realloc, [data_ptr, old_bytes, new_bytes])
  Store(Field(recv, 0), new_ptr)
  Store(Field(recv, 2), new_cap)
  Goto(copy_bb)

copy_bb: memcpy + update len
  current_ptr = Field(recv, 0)  (reload — handles growth)
  dest_ptr = GetElementPtr(current_ptr, [len], *mut u8)
  Call(__landin_memcpy, [dest_ptr, src_ptr, src_len])
  Store(Field(recv, 1), new_len)
  Goto(after)
```

**MVP scope (§17.6 record)**:
- **Always realloc**: libc `realloc(NULL, size) == malloc(size)` per C standard.
- **No OOM check**: `__landin_realloc` itself panics on OOM.
- **PHI avoidance**: Reload `str.ptr` in copy_bb via `Projection(recv, Field(0))`.
- **memcpy via C helper**: `__landin_memcpy` is a primitive C helper (per §16.5, not in
  migration scope). Used for the byte copy operation instead of a MIR loop.
- **Growth while loop**: Expressed via MIR back-edge (grow_loop_bb ↔ grow_body_bb).
  First MIR loop generated by an intrinsic (all previous intrinsics used straight-line code).

## 3. 任务审查结论 (per §17.8)

### 3.1 Is this the best time?

**Yes** — all dependencies ready (per §2.1 audit table — all ✅).
Stage 18.229 established the patterns for:
- Mutable PHI-like locals (`new_cap_local`)
- StatementKind::Store to Field projections
- SwitchInt for conditional growth dispatch
- Store Deref codegen

### 3.2 Risk Assessment

| Risk | Mitigation |
|------|-----------|
| MIR back-edge (loop) in intrinsic may confuse DCE | DCE only removes dead Assigns, not reachable blocks; loop body is reachable from grow_loop_bb |
| `new_cap_local` read in grow_loop_bb before any write in that block | Initialized in zero_cap_bb/nonzero_cap_bb (both predecessors of grow_loop_bb); borrowck's initialized set handles this |
| `__landin_memcpy` may not be declared | Already registered in driver_validations.rs (u32::MAX - 101) and function_sigs.rs |
| Growth calculation while loop may not terminate | Terminates because new_cap doubles each iteration and new_len is finite; max ~63 iterations for i64 |

## 4. Implementation Plan

### 4.1 Files to Modify

| File | Change | LOC (est.) |
|------|--------|-----------|
| `src/mir/lower/expr_variants.rs` | Rewrite `lower_string_push_str_intrinsic` | ~220 (replace ~110) |
| `docs/lang-design/06-mir.md` | Update §16.6 to mark v0.2.5f done | ~30 |
| `docs/develop/v0/stage-18/stage-18.230-dev-log.md` | New dev-log | ~100 |

### 4.2 Test Plan (per §9.4)

| Test | Category | Verification |
|------|----------|-------------|
| `stage18_198_push_str_append` | Regression | `s.push_str(" world")` on "hello" → len=11 |
| `stage18_198_push_str_from_empty` | Regression | `s.push_str("hello")` on empty → len=5 |
| `stage18_198_push_str_multiple` | Regression | Multiple pushes → len=13 |
| `stage18_198_push_str_growth` | Regression | Growth triggers (cap=16 after 3 pushes) |
| `stage18_198_push_str_empty_src` | Regression | `s.push_str("")` → len unchanged |
| `stage18_198_push_str_long` | Regression | Long string (43 bytes) → len=43 |

## 5. Recommendation

**Proceed with v0.2.5f migration** — `__landin_string_push_str` → MIR intrinsic sequence
with while loop for growth calculation + `__landin_memcpy` for byte copy.

All dependencies ready. MVP scope recorded. Next stage (v0.2.5g) will migrate
`__landin_format_variadic` (the most complex of the 4 C helpers).
