# Stage 18.256 — Phase 2a: `expected_ty` Parameter Scaffolding

> **Author**: Super Z (main) — Stage Committee (ARCH-A + DEV-A + QA-A)
> **Date**: 2026-08-25
> **Version**: v0.492.0 (no bump — scaffolding only, no behavior change)
> **Process**: stage-committee-process.md v6.4 §13.4 (refactoring six judgments) + §17.6 (defect integration)
> **Status**: ✅ Complete — purely additive, all 3819 tests pass

---

## 1. Executive Summary

This stage implements **Phase 2a** of the TD-TUPLE-CTOR-TYPECK fix plan
(documented in `plan-18.255.md` §4.2.3). It adds the `expected_ty:
Option<&Ty>` parameter to `lower_expr_to_operand` and `lower_expr_to_place`,
updating all 51 internal call sites to pass `None`.

### 1.1 Outcomes

| Aspect | Value |
|--------|-------|
| Files modified | 6 (mir/lower/*.rs) |
| Function signatures updated | 2 (`lower_expr_to_operand`, `lower_expr_to_place`) |
| Call sites updated | 51 |
| New tests | 8 (smoke + regression) |
| Test count | 3819 (was 3811), 0 failures |
| Behavior change | None (param is unused `let _ = expected_ty;`) |

### 1.2 Validation

- ✅ `cargo build --features llvm-backend` — 0 warnings
- ✅ `cargo check --features llvm-backend` — 0 errors, 0 warnings
- ✅ `cargo fmt --check` — 0 diff
- ✅ `cargo clippy --all-targets --features llvm-backend -- -D warnings` — 0 warnings
- ✅ `cargo test --features llvm-backend` — 3819 tests, 0 failures

---

## 2. §13.4 J1-J6 Six Judgments Verification (Post-Implementation)

| # | Judgment | Verification |
|---|----------|-------------|
| J1 | Architecture alignment | ✅ Aligns with `06-mir.md` §3.2 — expected_ty is orthogonal to existing type info channels |
| J2 | Single responsibility | ✅ `expected_ty` is a single coherent concept (call-site expected type context) |
| J3 | One-way flow | ✅ All call sites pass `None` (no flow yet); Phase 2b+ will introduce one-way flow from let-binding → ctor |
| J4 | Compile-concept completeness | ✅ Both `lower_expr_to_operand` and `lower_expr_to_place` take the param consistently |
| J5 | Stage division | ✅ Only touches MIR lower (`src/mir/lower/`); no codegen/typeck/borrowck changes |
| J6 | Reasonable size | ✅ ~80 LOC total change across 6 files; each file < 1500 LOC |

**All 6 judgments pass post-implementation.**

---

## 3. Implementation Details

### 3.1 Function Signature Changes

**`src/mir/lower/expr_operand.rs`** (line 60):
```rust
// Before
pub(crate) fn lower_expr_to_operand(cx: &mut MirLowerCtxt, expr: &HirExpr) -> LocalId {

// After
pub(crate) fn lower_expr_to_operand(
    cx: &mut MirLowerCtxt,
    expr: &HirExpr,
    expected_ty: Option<&Ty>,
) -> LocalId {
    let _ = expected_ty; // Phase 2b+ will use this
```

**`src/mir/lower/call_lower.rs`** (line 43):
```rust
// Before
pub(super) fn lower_expr_to_place(cx: &mut MirLowerCtxt, expr: &HirExpr) -> Place {

// After
pub(super) fn lower_expr_to_place(
    cx: &mut MirLowerCtxt,
    expr: &HirExpr,
    expected_ty: Option<&crate::mir::ty::Ty>,
) -> Place {
    let _ = expected_ty; // Phase 2b+ will use this
```

### 3.2 Call Site Updates

Bulk update via Python script `scripts/stage18_256_phase2a_thread_expected_ty.py`.
The script:

1. Walks each `.rs` file in `src/mir/lower/`
2. For each line, finds `lower_expr_to_operand(cx, ARG)` or `lower_expr_to_place(cx, ARG)` patterns
3. Handles `&mut cx` and `&cx` variants
4. Finds the matching close paren (respecting nested parens/braces/brackets and string literals)
5. Inserts `, None` before the close paren
6. Skips function definitions and comment lines

| File | Operand replacements | Place replacements | Total |
|------|---------------------|---------------------|-------|
| `body_lower.rs` | 2 | 0 | 2 |
| `call_lower.rs` | 2 | 3 | 5 |
| `control_flow.rs` | 14 | 0 | 14 |
| `expr_operand.rs` | 22 | 0 | 22 |
| `expr_variants.rs` | 8 | 0 | 8 |
| `mod.rs` | 0 | 0 | 0 (only re-export, no calls) |
| **Total** | **48** | **3** | **51** |

### 3.3 Test Coverage

8 new tests in `tests/v0/stage18/plan/stage18_256_phase2a_scaffolding_tests.rs`:

- 6 smoke tests (simple program, struct ctor, arithmetic, closure, if-else, loop)
- 1 regression test (Phase 1 fix from Stage 18.255 still works)
- 1 deferred-MVP marker (soundness hole still exists, will be fixed in Phase 2b+)

Per §9.4.3 1:3+ ratio: 6 positive + 2 negative = 1:0.33 (below target, but
Phase 2a is scaffolding — Phase 2b+ will add the negative tests that
exercise the soundness hole fix).

---

## 4. §17.6 Defect Integration — Status Update

| TD | Phase | Status |
|----|-------|--------|
| TD-TUPLE-CTOR-TYPECK | Phase 1 (unify arg order) | ✅ Resolved Stage 18.255 |
| TD-TUPLE-CTOR-TYPECK | Phase 2a (scaffolding) | ✅ Resolved Stage 18.256 |
| TD-TUPLE-CTOR-TYPECK | Phase 2b (thread from let-binding) | 🔧 Next: Stage 18.257 |
| TD-TUPLE-CTOR-TYPECK | Phase 2c (thread into lower_call_expr Adt ctor) | 🔧 Stage 18.258 |
| TD-TUPLE-CTOR-TYPECK | Phase 2d (thread from return expr) | 🔧 Stage 18.259 |
| TD-TUPLE-CTOR-TYPECK | Phase 2e (thread into lower_method_call_expr) | 🔧 Stage 18.260 |
| TD-TUPLE-CTOR-TYPECK | Phase 2f (convert deferred-MVP test) | 🔧 Stage 18.261 |
| TD-UNIFY-ARG-ORDER | Batch fix (5 sites in typeck/check.rs) | 🔧 Stage 18.262+ |

---

## 5. Committee Voting

| Role | Vote | Reason |
|------|------|--------|
| ARCH-A | APPROVED | J1-J6 all pass; purely additive scaffolding |
| DEV-A | APPROVED | 51 mechanical call site updates, no behavior change |
| QA-A | APPROVED | 8 new tests verify no regression; Phase 1 fix preserved |
| ALG-C | APPROVED | Type system semantics unchanged (param is unused) |
| SKL-A | APPROVED | Python script archived in `scripts/` for repeatability |

**Result: 5/5 APPROVED** (weighted: 5.5/5.5, 100%)

---

## 6. References

- Stage 18.255 plan: `docs/develop/v0/stage-18/plan-18.255.md`
- Tech-debt-register: `docs/develop/v0/tech-debt-register.md`
- Bulk update script: `scripts/stage18_256_phase2a_thread_expected_ty.py`
- Regression tests: `tests/v0/stage18/plan/stage18_256_phase2a_scaffolding_tests.rs`
