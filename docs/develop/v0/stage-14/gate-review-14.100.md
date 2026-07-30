# Stage 14.100 — Gate Review

> **Author**: redskaber
> **Date**: 2026-07-30
> **Version**: v0.113.0 → v0.114.0
> **Process**: stage-committee-process.md v3.22 §25 (D8 review)

## 1. Stage Summary

Stage 14.100 fixes 6 bugs found by the Round 8 independent audit:

- **Bug AA1**: `println!("{}", nonexistent)` silently printed 0
- **Bug AA2**: unresolved paths in for-loop body silently compiled
- **Bug AA3**: unresolved paths in Range silently used 0
- **Bug AA4**: unresolved paths in Repeat silently used 0
- **Bug AA5**: trait default body with zero impls crashed LLVM
- **Bug AA6**: Z7 fix too aggressive (false positive when both impls override)

All 6 are fully fixed. AA1-AA4 share the same root cause: incomplete scanning
of expression variants in `scan_expr_for_unresolved`. AA5 is a codegen
filter issue. AA6 is a refinement of Stage 14.99's Z7 check.

## 2. Bugs Fixed

### Bug AA1-AA4: Silent unresolved paths

**Symptoms**:
- AA1: `println!("{}", nonexistent_xyz)` silently printed 0
- AA2: `for i in 0..5 { let _ = nonexistent_xyz; }` silently compiled
- AA3: `for i in foo..5 { ... }` silently used foo=0
- AA4: `let arr = [foo; 3];` silently used foo=0

**Root cause**: `scan_expr_for_unresolved` in `src/driver.rs` had a
`_ => {}` catch-all that skipped:
- `HirExprKind::Println { args, .. }` — args contain paths
- `HirExprKind::For { iter, body, .. }` — iter and body contain paths
- `HirExprKind::Range { start, end, .. }` — start/end contain paths
- `HirExprKind::Repeat { elem, count }` — elem/count contain paths

Also, the Loop/While arm only scanned Expr statements, not Local statements
(within the body), so `loop { let _ = nonexistent; }` was also silently
accepted.

**Fix** (`src/driver.rs::scan_expr_for_unresolved`):
- Added explicit arm for `Println { args, .. }` — scan all args
- Added explicit arm for `For { iter, body, .. }` — scan iter + body stmts
  (Local + Expr) + trailing expr
- Added explicit arm for `Range { start, end, .. }` — scan start + end
- Added explicit arm for `Repeat { elem, count }` — scan elem + count
- Updated `Loop { body } | While { body }` arm to scan both Local and Expr
  statements (matching the For arm and the Block arm)

**Verification**:
- `println!("{}", nonexistent_xyz)` → resolve error ✅ (was: silent 0)
- `for i in 0..5 { let _ = nonexistent; }` → resolve error ✅ (was: silent)
- `for i in foo..5 { ... }` → resolve error ✅ (was: silent foo=0)
- `let arr = [foo; 3];` → resolve error ✅ (was: silent foo=0)

### Bug AA5: Trait default body with zero impls crashes LLVM

**Symptom**: `trait Shape { fn area(&self) -> i32; fn desc(&self) -> i32 { self.area() * 100 } }`
with zero impls crashed with `Function arguments must have first-class types! void %"%arg0"`.

**Root cause**: Trait default body methods are stored as `HirItem::Fn` owners
(after Stage 14.97 fix). When the trait has zero impls, the default body's
`self.<method>()` calls have no resolution, causing LLVM crashes. The body
was eagerly codegen'd even though never called.

**Fix** (`src/driver.rs`):
1. Track which body_ids are lowered (i.e., not skipped) in a
   `lowered_body_owners: HashSet<DefId>`.
2. Skip bodies that are trait default body methods when the trait has zero
   impls (check by iterating `hir.owners` for matching Trait + Impl).
3. Filter `body_metas` to only include lowered bodies — without this filter,
   codegen would try to emit functions for skipped bodies, producing invalid
   LLVM IR like `void %(void %arg0)`.

**Verification**:
- Trait with default body, zero impls → compiles and runs (no LLVM crash) ✅

### Bug AA6: Z7 false positive when both impls override

**Symptom**: Stage 14.99's Z7 check fired whenever a trait had a default body
method AND 2+ impls, regardless of whether any impl actually used the
default body. If both impls override the default, the default is never
called, so no specialization issue can occur.

**Fix** (`src/driver.rs`): Refine the Z7 check to only fire when at least
one impl does NOT override the default body method. For each trait method
with a body, check if every impl overrides it. If all override, skip the
error.

**Verification**:
- 2 impls, both override → no error, correct output (50, 100) ✅
- 2 impls, at least one doesn't override → error ✅ (Z7 still works)

## 3. Test Count Updates

| Suite | Before | After | Delta |
|-------|--------|-------|-------|
| Rust tests | 1951 | 1951 | 0 |
| Conformance tests | 5198 | 5204 | +6 |

New tests:
- `bk-0461-aa1-println-unresolved.lin` — AA1 (compile_error)
- `bk-0462-aa2-for-unresolved.lin` — AA2 (compile_error)
- `bk-0463-aa3-range-unresolved.lin` — AA3 (compile_error)
- `bk-0464-aa4-repeat-unresolved.lin` — AA4 (compile_error)
- `e2e-runok-169-trait-default-zero-impls.lin` — AA5 (run_ok)
- `e2e-runok-170-trait-default-multi-impl-override.lin` — AA6 (run_ok)

## 4. Verification

```
cargo build --release --features llvm-backend: ✅
cargo fmt: ✅ clean
cargo clippy --all-targets --features llvm-backend: ✅ 0 warnings
cargo test --features llvm-backend: ✅ 1951 passed, 0 failed
python3 tests/conformance/run_all.py: ✅ 5204 passed, 0 failed
```

## 5. Stage Verdict

**PASS** — All 6 bugs found by Round 8 audit are fully fixed. No regressions.
+6 new regression tests.

Per §1.0 原则 5 "报错 > 静默": AA1-AA4 now produce clear errors instead of
silent wrong output. AA5 was a crash (loud failure) — now works correctly.

Per §1.0 原则 6 "通用 > 特例": One unified `scan_expr_for_unresolved`
function handles all expression kinds — no per-kind special-casing.

Per §1.0 原则 1 "长期 > 短期": AA5 fix is at the right architectural layer
(driver-level body filtering), not a hack at codegen.

v0.114.0: minor bump (6 bug fixes — important correctness improvements)
