# Stage 15.89 — Trait Error Span Accuracy Fix

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.213.0 → v0.214.0
> **Process**: stage-committee-process.md v3.24 §29 + §9.3 + §25

## 1. Executive Summary

Stage 15.89 fixes the last `Span::DUMMY` error category: trait errors.
Previously, trait coherence errors ("conflicting implementations") and
incomplete impl errors ("missing method") both showed "1:1" (file start)
because the `CoherenceError` and `IncompleteImpl` structs had no `span`
field, and `to_diagnostics` hardcoded `Span::DUMMY`.

**Fix**:
1. Added `span: Span` field to `ImplInfo` (populated from `HirImpl.span`
   during `collect`).
2. Added `span: Span` field to `CoherenceError` (populated from the
   first conflicting impl's span during `check_coherence`).
3. Added `span: Span` field to `IncompleteImpl` (populated from the
   incomplete impl's span during `validate_impls`).
4. Updated `to_diagnostics` in `driver.rs` to use the trait error's span
   instead of `Span::DUMMY`.
5. Updated 16 test constructions of `ImplInfo` across 4 test files to
   include the new `span` field.

**Before** (`impl T for S {} impl T for S {}`):
```
error[E600]: conflicting implementations of trait `T` for type `S` (2 impl blocks)
  --> /tmp/t.lin:1:1
```

**After**:
```
error[E600]: conflicting implementations of trait `T` for type `S` (2 impl blocks)
  --> /tmp/t.lin:1:22
  |
1 | trait T {} struct S; impl T for S {} impl T for S {} fn main() {}
  |                      ^^^^
```

**Test impact**:
- 2 new Rust integration tests for span accuracy (coherence + incomplete)
- 0 conformance test changes
- **Total: 7596 tests passing** (236 lib + 2144 integration [was 2142,
  +2 new] + 5216 conformance), 0 failures, 0 warnings.

Per §1.0 原則 3 "显式 > 隐式": error spans are explicitly sourced from
the impl block, not defaulted to Span::DUMMY.
Per §1.0 原則 4 "报错 > 静默": error locations are accurate, not cryptic.

## 2. Why This Matters

Trait errors (coherence violations, incomplete impls) are common when
writing trait-based code. Previously, these errors showed "1:1" (file
start), making them hard to locate — the user had to search the whole
file for the problematic impl block.

This was the last remaining `Span::DUMMY` error category after the
9-stage error system cleanup (Stages 15.80-15.88 covered typeck,
borrowck, resolve, and MIR lowerer). Trait errors were missed because
the `CoherenceError`/`IncompleteImpl` structs predated the span accuracy
work and had no span field.

The fix completes the error system cleanup: ALL user-facing error
messages now point to actual source locations.

## 3. The Fix

### 3.1 `ImplInfo` span field (`src/traits/resolver.rs`)

```rust
pub struct ImplInfo {
    pub def_id: DefId,
    pub trait_name: Option<Spur>,
    pub self_ty_name: Option<Spur>,
    pub methods: Vec<Spur>,
    pub is_unsafe: bool,
    pub span: crate::session::Span,  // NEW
}
```

Populated in `collect` from `HirImpl.span` (which is always available
when processing `HirItem::Impl(i)`).

### 3.2 `CoherenceError` span field

```rust
pub struct CoherenceError {
    pub trait_name: Spur,
    pub self_ty_name: Spur,
    pub impl_def_ids: Vec<DefId>,
    pub span: crate::session::Span,  // NEW
}
```

Populated in `check_coherence` from the first conflicting impl's span.

### 3.3 `IncompleteImpl` span field

```rust
pub struct IncompleteImpl {
    pub trait_name: Spur,
    pub self_ty_name: Spur,
    pub missing_methods: Vec<Spur>,
    pub span: crate::session::Span,  // NEW
}
```

Populated in `validate_impls` from the incomplete impl's span.

### 3.4 `to_diagnostics` span fix (`src/driver.rs`)

```rust
// Before
for e in &self.trait_errors {
    diags.push(
        DiagnosticBuilder::error(&msg, crate::session::Span::DUMMY)
            .with_code(...)
            .build(),
    );
}

// After
for e in &self.trait_errors {
    let span = match e {
        TraitError::Coherence(ce) => ce.span,
        TraitError::Incomplete(inc) => inc.span,
    };
    diags.push(
        DiagnosticBuilder::error(&msg, span)
            .with_code(...)
            .build(),
    );
}
```

## 4. API Naming Compliance (§23)

**Struct field additions** (not new APIs, but new public fields):

| Struct | Field | §23 Compliance |
|--------|-------|-----------------|
| `ImplInfo` | `span: Span` | ✅ `<noun>` (property accessor) |
| `CoherenceError` | `span: Span` | ✅ `<noun>` |
| `IncompleteImpl` | `span: Span` | ✅ `<noun>` |

No new public functions. The `span` fields follow the naming convention
of existing span fields on `HirImpl`, `Place`, `Statement`, `Terminator`,
etc.

## 5. §16 Interface Isolation

The `span` field flows from `HirImpl.span` (HIR data) → `ImplInfo.span`
(trait resolver data) → `CoherenceError.span`/`IncompleteImpl.span`
(error data) → `to_diagnostics` (driver). This is a clean data flow
within the existing `traits` → `driver` dependency. No new cross-stage
dependencies.

The `collect` method already takes `&HirCrate`, so it has HIR access
(per §16.6 exception: resolver may read HIR during collect).

## 6. §25 Deep Review (8 Dimensions)

| Dimension | Status | Notes |
|-----------|--------|-------|
| D1 Architecture | ✅ | Span flows from HIR → resolver → error → diagnostics |
| D2 Tech Debt | ✅ | Last `Span::DUMMY` error category fixed (trait errors) |
| D3 Test Coverage | ✅ | 2 new integration tests verify span accuracy |
| D4 Next-Phase Readiness | ✅ | No regressions; ALL error categories now have accurate spans |
| D5 Design Rationality | ✅ | Mirrors span fields on other error types (TypeError, BorrowError) |
| D6 Performance | ✅ | One extra field copy per impl; negligible |
| D7 Documentation | ✅ | This doc + test plan + matrix updated |
| D8 Test Path Coverage | ✅ | Coherence + Incomplete paths have span accuracy tests |

**Committee Vote**: GO — Stage 15.89 complete.

## 7. Verification

- `cargo clean && cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 236/236 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2144/2144 PASS (was 2142, +2 new)
- `python3 tests/conformance/run_all.py` — ✅ 5216/5216 PASS
- **Total: 7596 tests passing, 0 failures, 0 warnings.**

## 8. Error System Cleanup Summary (Stages 15.80-15.89)

The ten-stage error system cleanup is now complete:

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
| 15.89 | Trait error span accuracy | 2 `Span::DUMMY` sites (last category) |
| **Total** | | **27 `Span::DUMMY` sites + 20 `{:?}` leaks fixed + 1 DRY** |

**Result**: ALL user-facing error messages (typeck, borrowck, resolve,
MIR lowerer, AND trait errors) now:
- Use human-readable type names (`i32`, `bool`, `&mut T`, etc.)
- Use human-readable region names (`'r5`, `'r2`)
- Use human-readable expression kind names (`"literal"`, `"function call"`)
- Point to actual source locations (with snippet underlines) — no
  `Span::DUMMY` / "1:1" errors remain

The error system is now fully cleaned up. Ready for user-facing work.

## 9. Next Steps

The error system cleanup is complete. The next major v0.2 task is:

**Task 12 (Lifetime elision)** — the next major v0.2 task (2-3 weeks,
P1, ready now). This is the last remaining P1 task for v0.2 release.

## 10. Version Policy

v0.213.0 → v0.214.0 (minor bump — Phase 2 trait error span accuracy
fix + 2 new integration tests).
