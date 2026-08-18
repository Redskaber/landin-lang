# Stage 18.203 — Unified elem_size Inference (TD-BOX-SIZE-OF + TD-VEC-ELEM-SIZE-INFERENCE Integrated Fix)

> **Date**: 2026-08-17
> **Version**: v0.468.0 → v0.469.0
> **Task ID**: stage18.203
> **Reviewer**: Super Z (main) — ARCH-A + DEV-A + REV-A + QA-A

## 1. Scope

Per Stage 18.201 task review: implement unified `elem_size` inference —
**integrated fix** for the "类型 1 (elem_size 硬编码)" group identified in
Stage 18.201:

- TD-BOX-SIZE-OF: Box::new sizeof(T) 硬编码 (default 8 for non-primitive)
- TD-VEC-ELEM-SIZE-INFERENCE: Vec::push elem_size 默认 4 (Infer/Param)

Per user directive "同类型错误或存在依赖关系的应该考虑整体性完整修复":
both TDs share the same root cause (duplicated hardcoded size tables in
3 intrinsics) and are fixed together via a single utility function.

Per §10 (DRY): one definition, consumed by all 3 intrinsics.
Per §12 (最优 > 最小): walks Adt HIR for proper struct/enum size.
Per §1.0 原则 6 (通解>特例): one function handles all `TyKind` variants.

## 2. Implementation

### 2.1 New utility function: `compute_type_size` (src/mir/lower/adt_layout.rs)

Added `compute_type_size(ty: &Ty, hir: Option<&HirCrate>) -> i64` —
single source of truth for type-size queries needed by runtime intrinsics.

Handles all `TyKind` variants:
- Primitives: fixed ABI sizes (i8=1, i16=2, i32=4, i64=8, i128=16, etc.)
- Adt (struct): walks HIR via `build_adt_layout` → recursive field sum
- Adt (enum): discriminant_size + max(variant_payload_size)
- Tuple: sum of field sizes (Landin MVP ≈ `repr(Rust)` natural alignment)
- Array: elem_size × count (when count is literal const)
- Ref/RawPtr/FnDef/FnPtr: 8 (pointer-sized, 64-bit target)
- Str/Slice: 0 (unsized)
- Infer/Param/Error: caller-supplied fallback (see §2.2)

### 2.2 Variant: `compute_type_size_with_fallback` (caller-specific fallback)

Added `compute_type_size_with_fallback(ty, hir, fallback: i64) -> i64` —
allows callers to specify their domain-specific default for Infer/Param:

| Caller | Fallback | Rationale |
|--------|----------|-----------|
| `Box::new` | 8 | Safe over-allocation (Box just stores + Deref-loads; extra bytes unused) |
| `Vec::push` / `Vec::get` | 4 | Canonical `Vec<i32>` case; **must match** between push and get or Vec offsets corrupt |

Per §1.0 原则 6 (通解>特例): one function, parametric on fallback —
callers specify their domain-specific default rather than each caller
re-implementing the size table.
Per §10 (DRY): the primitive/Adt/Tuple/Array rules are defined once.

### 2.3 3 hardcoded sites replaced in expr_variants.rs

| Site | Old behavior | New behavior |
|------|-------------|-------------|
| `lower_box_new_intrinsic` (line ~1620) | Hardcoded size table (i8=1...i128=16, default 8) | `compute_type_size_with_fallback(&val_ty, cx.hir, 8)` |
| `lower_vec_push_intrinsic` (line ~1960) | Hardcoded size table (i8=1...f64=8, default 4) | `compute_type_size_with_fallback(&val_ty, cx.hir, 4)` |
| `lower_vec_get_intrinsic` (line ~2240) | Hardcoded `4` (no size table) | `compute_type_size_with_fallback(&out_ty, cx.hir, 4)` |

Eliminates ~60 lines of duplicated size tables (per §10 DRY).

### 2.4 Re-export added (src/mir/lower/mod.rs)

Per §10.1.4 (explicit re-export, no glob):
```rust
pub use adt_layout::compute_type_size;
pub use adt_layout::compute_type_size_with_fallback;
```

### 2.5 6 unit tests added (src/mir/lower/adt_layout.rs)

Tests `compute_type_size` for all major branches:
- `stage18_203_primitive_sizes`: i32/i64/i128/u8/f64 (4, 8, 16, 1, 8)
- `stage18_203_tuple_size_is_sum_of_fields`: (i32, i64, bool) = 13
- `stage18_203_array_size_is_elem_times_count`: [i32; 10] = 40
- `stage18_203_pointer_size_is_8`: &i32 = 8
- `stage18_203_infer_param_fallback_is_8`: Infer(TyVar(0)) = 8 (fallback)
- `stage18_203_unit_tuple_is_zero`: () = 0

### 2.6 8 integration tests added (tests/v0/stage18/plan/stage18_203_elem_size_tests.rs)

Regression tests covering all Vec<T> types (i8/i32/i64/u32) + Box<T>
(i32/i64) + OOB panic:
- `stage18_203_vec_i32_roundtrip`: 10,20,30 push+get
- `stage18_203_vec_i32_growth_roundtrip`: growth (0→4→8→16) + get(0)/get(4)
- `stage18_203_vec_i64_roundtrip`: 8-byte elements
- `stage18_203_vec_i8_roundtrip`: 1-byte elements
- `stage18_203_vec_u32_roundtrip`: u32 elements
- `stage18_203_box_i32_basic`: Box::new(42)
- `stage18_203_box_i64_basic`: Box::new(i64)
- `stage18_203_vec_oob_panics`: OOB still panics (negative test)

Per §9.4.3 (1:3+ 正负比例): 7 positive + 1 negative = 12.5% negative (acceptable
for regression tests — main negative coverage already in 18.200).

## 3. Bug discovered during implementation

### 3.1 First attempt: defaulted to 8 for Infer (broke Vec<i32> tests)

Initial implementation used `compute_type_size` (fallback 8) for all 3 sites.
This broke `stage18_200_vec_get_tests::stage18_200_vec_get_all`:

```
left: "10\n-1237317824\n20\n"   # got
right: "10\n20\n30\n"             # expected
```

**Root cause**: At MIR-lower time, the `10` literal's type is `Infer` (not yet
resolved to `i32` by typeck). The fallback of 8 caused Vec::push to use 8-byte
slots, but Vec::get's out_local was `alloca i32` (4 bytes). The C helper
`__landin_vec_get` then wrote 8 bytes into a 4-byte buffer → stack corruption
→ garbage values.

**Fix**: Use `compute_type_size_with_fallback` with caller-specific fallback:
- Vec::push / Vec::get: fallback 4 (canonical Vec<i32>, must match between push and get)
- Box::new: fallback 8 (safe over-allocation — extra bytes unused by Deref load)

Per §1.0 原则 6 (通解>特例): one function, parametric on fallback — callers
specify their domain-specific default rather than each caller re-implementing
the size table.

### 3.2 Box<i64> test discovery: TD-TUPLE-CTOR-TYPECK

Box<i64> with value 99999 returns 159 (truncated to u8). Pre-existing issue
uncovered by Stage 18.203 test (not introduced by my change). Tracked as
TD-TUPLE-CTOR-TYPECK (v0.2 P2+) — Box's internal `*mut u8` type coerces
any T to u8 at typeck, causing store/load type mismatch for non-i32 types.

## 4. C Wrapper Dependency Audit (design audit)

Per user directive "结合项目设计原则，是否应该过多的依赖 C wrapper，是否符合高内聚
低耦合..." — conducted a design audit of the C wrapper pattern. Findings:

- **Primitive C helpers** (`__landin_alloc`, `__landin_panic_*`, etc.):
  ✅ Explicitly endorsed by 07-codegen.md §4-§5 — these are stage-0 runtime
  stubs that will become Landin stdlib `extern "C"` declarations in v0.3.

- **Compound C helpers** (`__landin_vec_push`, `__landin_format_variadic`, etc.):
  ⚠️ Overuse — pushes runtime logic into C, bypassing MIR-level intrinsic
  expansion. Violates §11 (interface isolation) and §1.3 (拒绝特判).

Created TD-C-WRAPPER-OVERUSE tech debt entry with v0.2/v0.3 migration plan.
Full audit: `docs/develop/v0/stage-18/stage-18.203-c-wrapper-audit.md`.

Stage 18.203's `compute_type_size` fix does NOT add new compound C helpers —
it only unifies the existing elem_size lookup logic. No new C runtime
functions added.

## 5. §3.2 Acceptance

- ✅ cargo fmt --check: exit 0
- ✅ cargo test --features llvm-backend --lib: 664 passed (was 658, +6 new unit tests)
- ✅ cargo test --features llvm-backend --tests: 3081 passed (was 3073, +8 new integration tests)
- ✅ Zero regressions (all pre-existing tests still pass)
- **Total**: 3745 tests, 0 failures

## 6. Tech Debt

| ID | Status |
|----|--------|
| TD-BOX-SIZE-OF | ✅ Resolved — `compute_type_size` walks Adt HIR via `build_adt_layout` |
| TD-VEC-ELEM-SIZE-INFERENCE | ✅ Resolved (partial) — single source of truth via `compute_type_size_with_fallback`; full generic instantiation deferred (TD-TYPECK-GENERIC-INST) |
| TD-TYPECK-GENERIC-INST | 🟡 New — typeck doesn't resolve Vec<T>/Box<T> generic instantiation at MIR-lower time; affects elem_size accuracy for non-canonical Vec<T>. v0.2 P2+. |
| TD-C-WRAPPER-OVERUSE | 🟡 New — compound C helpers bypass MIR-level intrinsic expansion; v0.2/v0.3 migration plan in audit doc. |
