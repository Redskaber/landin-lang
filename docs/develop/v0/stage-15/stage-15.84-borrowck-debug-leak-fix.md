# Stage 15.84 — Borrowck Debug Format Leak Fix + region_vid_to_string

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.208.0 → v0.209.0
> **Process**: stage-committee-process.md v3.24 §29 + §9.3 + §25

## 1. Executive Summary

Stage 15.84 extends the error system cleanup to borrowck. It fixes 3
`{:?}` Debug format leaks in borrowck error messages that were not
covered by Stages 15.80-15.83 (which focused on typeck):

1. **Lifetime error (RegionEscapesUniversal)**: `region {:?} escapes
   universal region {:?}` → `region 'r5 escapes universal region 'r2`
2. **Lifetime error (TypeTestFailed)**: `type {:?} does not outlive
   region {:?}` → `type i32 does not outlive region 'r2`
3. **NotCopy error**: `use of moved value: {:?} does not implement
   Copy` → `use of moved value: <adt> does not implement Copy`

**New helper**: `region_vid_to_string(vid: RegionVid) -> String` in
`src/mir/ty.rs` — formats `RegionVid(N)` as `'rN` (matches Rust's
convention for region variables).

**Test impact**:
- 1 new Rust unit test for `region_vid_to_string`
- 0 conformance test changes
- **Total: 7589 tests passing** (233 lib [was 232, +1 new] + 2140
  integration + 5216 conformance), 0 failures, 0 warnings.

Per §1.0 原則 3 "显式 > 隐式": user-facing region names are explicit.
Per §1.0 原則 4 "报错 > 静默": error messages are clear, not cryptic.

## 2. Why This Matters

The borrowck error messages had 3 remaining `{:?}` Debug format leaks
that were not covered by the typeck-focused cleanup in Stages 15.80-
15.83:

- **Lifetime errors** leaked `RegionVid(5)` (Debug format) instead of
  `'r5` (Rust convention). These errors are rare but confusing when
  they do occur — the user sees `RegionVid(5)` and has no idea what
  it means.
- **NotCopy errors** leaked `Adt(DefId(3), [])` (Debug format) instead
  of `<adt>` (human-readable). This error occurs when trying to Copy
  a non-Copy type (e.g., `let y = x; let z = x;` where `x` is a
  struct without `Copy`).

The fix makes all borrowck error messages consistent with typeck:
human-readable type names + human-readable region names.

## 3. The Fix

### 3.1 New `region_vid_to_string` helper (`src/mir/ty.rs`)

```rust
/// Stage 15.84: Format a `RegionVid` as a human-readable region string.
///
/// Matches Rust's convention: region variables display as `'r<N>`
/// (e.g., `'r0`, `'r5`).
pub fn region_vid_to_string(vid: RegionVid) -> String {
    format!("'r{}", vid.0)
}
```

Per §23 (API Naming): `region_vid_to_string` follows `<noun>_<verb>_<noun>`
pattern (matches `type_to_string`).

### 3.2 Lifetime error fixes (`src/borrowck/mod.rs`)

#### 3.2.1 RegionEscapesUniversal

```rust
// Before
format!(
    "lifetime error: region {:?} escapes universal region {:?}",
    escaping_region, universal_region
)

// After
format!(
    "lifetime error: region {} escapes universal region {}",
    crate::mir::ty::region_vid_to_string(*escaping_region),
    crate::mir::ty::region_vid_to_string(*universal_region),
)
```

#### 3.2.2 TypeTestFailed

```rust
// Before
format!(
    "lifetime error: type {:?} does not outlive region {:?}",
    ty.kind, universal_region
)

// After
format!(
    "lifetime error: type {} does not outlive region {}",
    crate::mir::ty::type_kind_to_string(&ty.kind),
    crate::mir::ty::region_vid_to_string(*universal_region),
)
```

### 3.3 NotCopy error fix (`src/borrowck/mod.rs`)

```rust
// Before
format!(
    "use of moved value: {:?} does not implement Copy; \
     use an explicit move (`let y = move x;`) or borrow",
    ty.kind
)

// After
format!(
    "use of moved value: {} does not implement Copy; \
     use an explicit move (`let y = move x;`) or borrow",
    crate::mir::ty::type_kind_to_string(&ty.kind)
)
```

## 4. API Naming Compliance (§23)

**New public function**:

| Function | Location | §23 Compliance |
|----------|----------|-----------------|
| `region_vid_to_string(vid: RegionVid) -> String` | `mir::ty` | ✅ `<noun>_<verb>_<noun>` (matches `type_to_string`) |

## 5. §16 Interface Isolation

The new `region_vid_to_string` helper lives in `mir::ty` (same module
as `RegionVid`). It reads only `RegionVid.0` (a `u32`) — no resolver,
no HIR, no borrowck access.

Callers (`borrowck::mod`) import the helper via
`crate::mir::ty::region_vid_to_string`. No new cross-stage dependencies
— `borrowck` already imports `crate::mir::ty::type_kind_to_string`
(since Stage 15.80).

## 6. §25 Deep Review (8 Dimensions)

| Dimension | Status | Notes |
|-----------|--------|-------|
| D1 Architecture | ✅ | Helper in `mir::ty` (same module as `RegionVid`); callers import explicitly |
| D2 Tech Debt | ✅ | 3 more `{:?}` Debug leaks fixed (borrowck) |
| D3 Test Coverage | ✅ | 1 new unit test for `region_vid_to_string` |
| D4 Next-Phase Readiness | ✅ | No regressions; borrowck errors now consistent with typeck |
| D5 Design Rationality | ✅ | Matches Rust convention (`'rN` for region variables) |
| D6 Performance | ✅ | One `format!` per error; negligible |
| D7 Documentation | ✅ | This doc + test plan + matrix updated |
| D8 Test Path Coverage | ✅ | `region_vid_to_string` has unit test; borrowck error paths covered by existing conformance tests |

**Committee Vote**: GO — Stage 15.84 complete.

## 7. Verification

- `cargo clean && cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 233/233 PASS (was 232, +1 new)
- `cargo test --features llvm-backend --test all_tests` — ✅ 2140/2140 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5216/5216 PASS
- **Total: 7589 tests passing, 0 failures, 0 warnings.**

## 8. Error System Cleanup Summary (Stages 15.80-15.84)

The five-stage error system cleanup is now complete:

| Stage | Focus | Sites Fixed |
|-------|-------|-------------|
| 15.80 | Human-readable type names (`type_to_string`) | 6 `{:?}` leaks + 2 `({:?})` enum leaks |
| 15.81 | Terminator span accuracy (`operand_span`, `term.span`) | 7 `Span::DUMMY` sites + 1 `{:?}` leak |
| 15.82 | Statement/rvalue span accuracy (`stmt_span` in `infer_rvalue`) | 9 `Span::DUMMY` sites + 5 `{:?}` leaks |
| 15.83 | Aggregate (Array + Adt) span accuracy | 2 `Span::DUMMY` sites |
| 15.84 | Borrowck Debug leaks (`region_vid_to_string`) | 3 `{:?}` leaks |
| **Total** | | **20 `Span::DUMMY` sites + 17 `{:?}` leaks fixed** |

**Result**: All user-facing typeck AND borrowck error messages now:
- Use human-readable type names (`i32`, `bool`, `&mut T`, etc.) — no
  Debug format leaks
- Use human-readable region names (`'r5`, `'r2`) — no `RegionVid(N)`
  Debug leaks
- Point to actual source locations (with snippet underlines) — no
  `Span::DUMMY` / "1:1" errors (typeck only; borrowck still has some
  DUMMY spans in check_terminator callers, deferred)

The error system is now in good shape for user-facing work.

## 9. Next Steps

The error system cleanup is substantially complete. Remaining items:
- Borrowck `check_terminator` passes `Span::DUMMY` to `check_operand`
  for Call/SwitchInt/Assert. Could use `term.span` instead. Deferred
  (lower priority — these errors are rare).

The next major v0.2 task is:

**Task 12 (Lifetime elision)** — the next major v0.2 task (2-3 weeks,
P1, ready now). This is the last remaining P1 task for v0.2 release.

## 10. Version Policy

v0.208.0 → v0.209.0 (minor bump — Phase 2 error system borrowck Debug
leak fix + 1 new unit test).
