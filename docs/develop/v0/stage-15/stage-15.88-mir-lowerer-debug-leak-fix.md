# Stage 15.88 — MIR Lowerer Debug Leak Fix + hir_expr_kind_to_string

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.212.0 → v0.213.0
> **Process**: stage-committee-process.md v3.24 §29 + §9.3 + §25

## 1. Executive Summary

Stage 15.88 extends the error system cleanup to the MIR lowerer. It
fixes 3 `{:?}` Debug format leaks in MIR lowering error messages that
were not covered by Stages 15.80-15.87 (which focused on typeck,
borrowck, and resolve):

1. **"no method found" error**: `no method `{}` found for type `{:?}``
   → `no method `{}` found for type `<adt>`` (uses `type_kind_to_string`)
2. **"for-loop only supports Range" error**: `found {:?}` →
   `found {}` (uses new `hir_expr_kind_to_string`)
3. **"array repeat count" error**: `(found {:?})` →
   `(found {})` (uses new `hir_expr_kind_to_string`)

**New helper**: `hir_expr_kind_to_string(kind: &HirExprKind) -> &'static str`
in `src/hir/kinds.rs` — formats `HirExprKind` as a human-readable label
(e.g., `"literal"`, `"function call"`, `"range expression"`) instead of
Debug format.

**Test impact**:
- 1 new Rust unit test for `hir_expr_kind_to_string` (in `hir::kinds::tests`)
- 1 new Rust integration test for "no method found" human-readable type name
- 0 conformance test changes
- **Total: 7594 tests passing** (236 lib [was 235, +1 new] + 2142
  integration [was 2141, +1 new] + 5216 conformance), 0 failures, 0
  warnings.

Per §1.0 原則 3 "显式 > 隐式": user-facing expression kind names are explicit.
Per §1.0 原則 4 "报错 > 静默": error messages are clear, not cryptic.

## 2. Why This Matters

The MIR lowerer produces type errors for:
- Method calls on types without the method (`s.f()` where `S` has no `f`)
- For-loop with non-Range iterator (`for x in arr { ... }` where `arr`
  is an array, not a Range)
- Array repeat with non-literal count (`[0; n]` where `n` is a variable)

Previously, these errors leaked Debug format:
- `Adt(DefId(1), [])` for the receiver type in "no method found"
- `HirExprKind::Path(...)` for the iterator in "for-loop only supports Range"
- `HirExprKind::Path(...)` for the count in "array repeat count"

The fix makes these error messages consistent with the typeck/borrowck
cleanup (Stages 15.80-15.87): human-readable type names + human-readable
expression kind names.

## 3. The Fix

### 3.1 New `hir_expr_kind_to_string` helper (`src/hir/kinds.rs`)

```rust
pub fn hir_expr_kind_to_string(kind: &HirExprKind) -> &'static str {
    match kind {
        HirExprKind::Lit(_) => "literal",
        HirExprKind::Path(_) => "path",
        HirExprKind::Block(_) => "block",
        HirExprKind::Call { .. } => "function call",
        HirExprKind::MethodCall { .. } => "method call",
        HirExprKind::Field { .. } => "field access",
        HirExprKind::Index { .. } => "index",
        HirExprKind::Unary { .. } => "unary op",
        HirExprKind::Binary { .. } => "binary op",
        HirExprKind::Assign { .. } => "assignment",
        HirExprKind::AddrOf { .. } => "borrow",
        HirExprKind::Cast { .. } => "cast",
        ...
    }
}
```

Per §23 (API Naming): `hir_expr_kind_to_string` follows
`<noun>_<verb>_<noun>` pattern (matches `type_kind_to_string`).

Re-exported from `hir::mod` as `pub use kinds::hir_expr_kind_to_string`.

### 3.2 "no method found" error fix (`src/mir/lower/expr_operand.rs`)

```rust
// Before
format!("no method `{}` found for type `{:?}`", method_name_str, recv_ty.kind)

// After
format!(
    "no method `{}` found for type `{}`",
    method_name_str,
    crate::mir::ty::type_kind_to_string(&recv_ty.kind)
)
```

### 3.3 "for-loop only supports Range" error fix

```rust
// Before
format!("for-loop only supports Range iterators ...; found {:?}", iter.kind)

// After
format!(
    "for-loop only supports Range iterators ...; found {}",
    crate::hir::hir_expr_kind_to_string(&iter.kind)
)
```

### 3.4 "array repeat count" error fix

```rust
// Before
format!("array repeat count must be a literal integer ...; (found {:?})", count.kind)

// After
format!(
    "array repeat count must be a literal integer ...; (found {})",
    crate::hir::hir_expr_kind_to_string(&count.kind)
)
```

## 4. API Naming Compliance (§23)

**New public function**:

| Function | Location | §23 Compliance |
|----------|----------|-----------------|
| `hir_expr_kind_to_string(kind: &HirExprKind) -> &'static str` | `hir::kinds` (re-exported from `hir`) | ✅ `<noun>_<verb>_<noun>` (matches `type_kind_to_string`) |

## 5. §16 Interface Isolation

The new `hir_expr_kind_to_string` helper lives in `hir::kinds` (same
module as `HirExprKind`). It reads only `HirExprKind` data — no MIR,
no resolver, no typeck access.

Callers (`mir::lower::expr_operand`) import via
`crate::hir::hir_expr_kind_to_string`. This is a clean dependency:
`mir::lower` already depends on `hir` (it lowers HIR to MIR). No new
cross-stage dependencies.

## 6. §25 Deep Review (8 Dimensions)

| Dimension | Status | Notes |
|-----------|--------|-------|
| D1 Architecture | ✅ | Helper in `hir::kinds` (same module as `HirExprKind`); callers import explicitly |
| D2 Tech Debt | ✅ | 3 more `{:?}` Debug leaks fixed (MIR lowerer) |
| D3 Test Coverage | ✅ | 1 new unit test + 1 new integration test |
| D4 Next-Phase Readiness | ✅ | No regressions; MIR lowerer errors now consistent with typeck/borrowck |
| D5 Design Rationality | ✅ | Mirrors `type_kind_to_string` pattern (Stage 15.80) |
| D6 Performance | ✅ | One `match` per error; negligible |
| D7 Documentation | ✅ | This doc + test plan + matrix updated |
| D8 Test Path Coverage | ✅ | `hir_expr_kind_to_string` has unit test; "no method found" path has integration test |

**Committee Vote**: GO — Stage 15.88 complete.

## 7. Verification

- `cargo clean && cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 236/236 PASS (was 235, +1 new)
- `cargo test --features llvm-backend --test all_tests` — ✅ 2142/2142 PASS (was 2141, +1 new)
- `python3 tests/conformance/run_all.py` — ✅ 5216/5216 PASS
- **Total: 7594 tests passing, 0 failures, 0 warnings.**

## 8. Error System Cleanup Summary (Stages 15.80-15.88)

The nine-stage error system cleanup is now complete:

| Stage | Focus | Sites Fixed |
|-------|-------|-------------|
| 15.80 | Human-readable type names (`type_to_string`) | 6 `{:?}` leaks + 2 `({:?})` enum leaks |
| 15.81 | Typeck terminator span accuracy | 7 `Span::DUMMY` sites + 1 `{:?}` leak |
| 15.82 | Typeck statement/rvalue span accuracy | 9 `Span::DUMMY` sites + 5 `{:?}` leaks |
| 15.83 | Typeck aggregate span accuracy | 2 `Span::DUMMY` sites |
| 15.84 | Borrowck Debug leaks (`region_vid_to_string`) | 3 `{:?}` leaks |
| 15.85 | Borrowck terminator span accuracy | 4 `Span::DUMMY` sites |
| 15.86 | DRY refactor: unify `operand_span` | 1 duplicate eliminated |
| 15.87 | Resolve error span accuracy | 1 `Span::DUMMY` site |
| 15.88 | MIR lowerer Debug leaks (`hir_expr_kind_to_string`) | 3 `{:?}` leaks |
| **Total** | | **25 `Span::DUMMY` sites + 20 `{:?}` leaks fixed + 1 DRY** |

**Result**: All user-facing typeck, borrowck, resolve, AND MIR lowerer
error messages now use human-readable type names, region names, and
expression kind names — no Debug format leaks.

## 9. Next Steps

The error system cleanup is complete. The next major v0.2 task is:

**Task 12 (Lifetime elision)** — the next major v0.2 task (2-3 weeks,
P1, ready now). This is the last remaining P1 task for v0.2 release.

## 10. Version Policy

v0.212.0 → v0.213.0 (minor bump — Phase 2 MIR lowerer Debug leak fix +
1 new helper + 2 new tests).
