# Stage 15.11 — Const.ty Box<Ty> → Ty

> **Author**: redskaber
> **Date**: 2026-07-31
> **Version**: v0.136.0 → v0.137.0
> **Process**: stage-committee-process.md v3.23 §29 (stage-end deep review)

## 1. Executive Summary

Stage 15.11 changes `Const.ty` from `Box<Ty>` to `Ty` — eliminating a heap
allocation per `Const`. `Ty` is already a small struct (one `TyKind` field),
so `Box<Ty>` was an unnecessary indirection. The `Box` was originally added
to break a recursive type cycle, but `Const` is only referenced via
`Box<Const>` (in `TyKind::Array(Box<Ty>, Box<Const>)`), so the cycle is
already broken — the `Box` on `Const.ty` is redundant.

For a crate with 100 constants (typical for a 100-fn crate), this eliminates
100 heap allocations per compilation.

## 2. Why This Change?

Per Phase 2 audit:
- `Const.ty: Box<Ty>` causes per-Const heap allocation
- Every `Const { ty: Box::new(...), val: ... }` allocates a Box
- `Ty` is small (one `TyKind` field, ~40 bytes) — no benefit from Box

The `Box<Ty>` was originally added to avoid recursive type issues, but
analysis shows `Const` is only referenced via `Box<Const>` (in
`TyKind::Array(Box<Ty>, Box<Const>)`), so the recursive cycle
(`Ty → TyKind::Array → Const → Const.ty → Ty`) is already broken by the
`Box<Const>`. The `Box<Ty>` on `Const.ty` is redundant.

**Note on Sig.output**: Stage 15.11 also considered changing `Sig.output:
Box<Ty>` to `Ty`, but this would cause a recursive type error because
`TyKind::FnPtr(Sig)` → `Sig.output: Ty` → `Ty { kind: TyKind::FnPtr(Sig) }`
is an infinite-size cycle. The `Box<Ty>` on `Sig.output` is structurally
necessary until full Ty interning (v0.3) makes `Ty` a thin pointer.

## 3. Design

### 3.1 Type change

```rust
// Before (Stage 3.47 - 15.10):
pub struct Const {
    pub ty: Box<Ty>,
    pub val: ConstVal,
}

// After (Stage 15.11):
pub struct Const {
    pub ty: Ty,  // inline, no Box
    pub val: ConstVal,
}
```

### 3.2 Construction patterns

| Pattern | Before | After |
|---------|--------|-------|
| New Const | `Const { ty: Box::new(Ty::new(kind, span)), val }` | `Const { ty: Ty::new(kind, span), val }` |
| From existing Ty | `Const { ty: Box::new(ty), val }` | `Const { ty: ty, val }` |

55 construction sites were updated across 8 source files + 1 test file.

### 3.3 Consumption patterns

| Pattern | Before | After |
|---------|--------|-------|
| Get Ty ref | `c.ty.as_ref()` | `&c.ty` (or just `c.ty` in many contexts) |
| Clone Ty | `c.ty.as_ref().clone()` | `c.ty.clone()` |
| Pattern match | `match &c.ty.kind { ... }` (unchanged — Box derefs) | `match &c.ty.kind { ... }` (unchanged) |

3 consumption sites were updated (all `c.ty.as_ref().clone()` → `c.ty.clone()`).

### 3.4 No recursive type issue

`Const` is referenced in `TyKind::Array(Box<Ty>, Box<Const>)`. The
`Box<Const>` breaks the recursive cycle:
- `Ty` → `TyKind::Array(Box<Ty>, Box<Const>)` → `Box<Const>` → `Const`
- `Const.ty: Ty` → `Ty` → (cycle, but broken by `Box<Const>`)

Without the `Box<Const>`, `Const.ty: Ty` would cause infinite size.
With `Box<Const>`, the cycle is broken and `Const.ty: Ty` is safe.

## 4. §29 Stage-End Deep Review

### 4.1 Data flow coverage (§29.1.1)

Data flow is unchanged — `Const.ty` is still a `Ty` value. The only
difference is ownership: `Box<Ty>` (heap-allocated) → `Ty` (inline).
Consumption patterns (`&c.ty`, `c.ty.clone()`, `c.ty.kind`) are identical
or simpler.

### 4.2 Architecture review (§29.1.2)

**Structural clarity** ✅ — `Const.ty` is now `Ty`, matching the pattern
of `LocalDecl.ty`, `Statement` fields, etc. (all `Ty`, not `Box<Ty>`).

**Efficiency** ✅ — eliminates per-Const heap allocation. `Ty` is ~40 bytes
(inline in Const) vs `Box<Ty>` (8-byte pointer + 40-byte heap allocation).

**Extensibility** ✅ — future v0.3 Ty interning (`Ty(Rc<TyKind>)`) will
make `Ty` 8 bytes, further reducing Const size.

### 4.3 Design-impl-test coverage (§29.1.3)

| Design point | Implementation | Test |
|--------------|----------------|------|
| Integer Const construction | `Const { ty: Ty::new(...), val: Int(42) }` | `stage15_11_integer_constant` |
| Boolean Const construction | `Const { ty: Ty::new(Bool), val: Bool(true) }` | `stage15_11_boolean_constant` |
| Function call with Const arg | Call terminator with Const operand | `stage15_11_function_call_with_constant` |
| Binary op with Consts | BinaryOp with Constant operands | `stage15_11_binary_op_with_constants` |
| Array with Const elements | Aggregate(Array, [Const, ...]) | `stage15_11_array_with_constants` |
| Method call with Const | Method call on Const-typed local | `stage15_11_method_call_with_constant` |
| Match with Const discriminant | Switch with Const discriminant operand | `stage15_11_match_with_constant` |

### 4.4 Hidden problems (§29.1.4)

| Hidden problem | Complexity growth | Stage 15.11 status |
|----------------|-------------------|-------------------|
| `Sig.output: Box<Ty>` still has Box | 1× (structurally necessary) | Documented — deferred to v0.3 |
| `Const.ty` is now inline (40 bytes in Const) | 1× (Const grows by 32 bytes) | Acceptable — Const is rare |
| No deduplication of Const values | 2× (v0.3 const interning) | Deferred to v0.3 |

No new hidden problems. The Ty inline is the standard Rust pattern for
small types.

### 4.5 Refactoring optimality (§29.2)

**Approach taken** ✅ — `Const.ty: Ty` is the standard Rust pattern for
small types. The Box was unnecessary because `Const` is already boxed at
its reference site (`Box<Const>` in `TyKind::Array`).

**Alternative considered** ✅ — Could have also changed `Sig.output:
Box<Ty>` to `Ty`. Rejected because it causes a recursive type error
(`TyKind::FnPtr(Sig)` → `Sig.output: Ty` → cycle). The Box on Sig.output
is structurally necessary until v0.3 Ty interning.

**Skipped refactors** ✅ — Did not change `TyKind::Ref(Box<Ty>)`,
`TyKind::RawPtr(Box<Ty>)`, etc. to `Ty`. Those Boxes ARE structurally
necessary (recursive type cycle: `Ty → TyKind::Ref(Box<Ty>) → Ty`).
Per §15 "最优 > 最小": only change what's safe.

## 5. Test Results

| Test suite | Before (v0.136.0) | After (v0.137.0) | Delta |
|------------|-------------------|------------------|-------|
| Rust unit (lib) | 145 | 145 | 0 |
| Rust integration (all_tests) | 1983 | 1990 | +7 (Const.ty tests) |
| Conformance (.lin) | 5216 | 5216 | 0 |
| **Total** | **7344** | **7351** | **+7** |

All tests pass. Zero regressions. Zero clippy warnings. fmt clean.

## 6. Files Changed

| File | Change |
|------|--------|
| `Cargo.toml` | Version bump 0.136.0 → 0.137.0 |
| `src/mir/ty.rs` | `Const.ty: Box<Ty>` → `Ty`; added Stage 15.11 doc on Sig.output (why Box is kept) |
| `src/typeck/checker.rs` | 3 `ty: Box::new(X)` → `ty: X`; 2 `c.ty.as_ref().clone()` → `c.ty.clone()` |
| `src/mir/body.rs` | 1 `ty: Box::new(X)` → `ty: X` |
| `src/mir/place.rs` | 2 `ty: Box::new(X)` → `ty: X` |
| `src/mir/lower/expr_operand.rs` | 16 `ty: Box::new(X)` → `ty: X` |
| `src/mir/lower/control_flow.rs` | 10 `ty: Box::new(X)` → `ty: X` |
| `src/mir/lower/overflow_assert.rs` | 2 `ty: Box::new(X)` → `ty: X` |
| `src/mir/lower/mod.rs` | 11 `ty: Box::new(X)` → `ty: X` |
| `src/mir/lower/writeback.rs` | 1 `c.ty.as_ref().clone()` → `c.ty.clone()` |
| `src/borrowck/mod.rs` | 9 `ty: Box::new(X)` → `ty: X` |
| `tests/v0/stage5/plan/codegen_dyn_trait_method_call_tests.rs` | 1 `ty: Box::new(X)` → `ty: X` |
| `tests/v0/stage15/plan/const_ty_box_to_ty_tests.rs` | **NEW** — 7 integration tests |
| `tests/all_tests.rs` | Registered `stage15_const_ty_box_to_ty_tests` |
| `docs/develop/v0/stage-15/stage-15.11-const-ty-box-to-ty.md` | This document |
| `docs/tests/v0/stage15/stage-15.11-test-plan.md` | **NEW** — test plan |
| `docs/worklog.md` | Stage 15.11 entry appended |
| `RELEASE_NOTES.md` | v0.137.0 entry appended |
| `README.md` | Updated with Stage 15.11 progress |
