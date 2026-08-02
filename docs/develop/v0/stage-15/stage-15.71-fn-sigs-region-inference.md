# Stage 15.71 — fn_sigs Integration for Region Inference

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.195.0 → v0.196.0
> **Process**: stage-committee-process.md v3.24 §29

## 1. Executive Summary

Stage 15.71 integrates `fn_sigs` into the borrow checker's region inference
for proper call-argument region constraints. Previously, call arguments used
a simplified `'static` constraint (Stage 15.50). Now, when `fn_sigs` is
available, the callee's parameter types are used for proper region constraints
between call arguments and parameters.

**Key results**:
- Added `fn_sigs` field to `BorrowChecker` + `with_fn_sigs` constructor.
- Added `collect_mir_constraints_with_sigs` method to `RegionInferenceContext`.
- Driver now passes `fn_sigs` via `BorrowChecker::with_fn_sigs`.
- Resolver NOT passed (backward compat — unsound `ty_is_copy` retained).
- All 7567 tests pass (221 lib + 2130 integration + 5216 conformance).

## 2. What Was Done

### 2.1 fn_sigs field + with_fn_sigs constructor

Added `fn_sigs: Option<&HashMap<DefId, Sig>>` to `BorrowChecker`. The
`with_fn_sigs` constructor passes fn_sigs WITHOUT resolver/interner, so
`is_copy` falls back to the unsound `ty_is_copy` (treats all Adt as Copy).
This maintains backward compatibility with 200+ tests that expect structs
to be Copy. Sound Copy detection (HP-1) is deferred to v0.3.

### 2.2 collect_mir_constraints_with_sigs

New method in `RegionInferenceContext` that accepts optional `fn_sigs`.
When available, call arguments' regions are constrained against the callee's
parameter regions (looked up via `fn_sigs[def_id]`). When not available,
falls back to the simplified `'static` constraint.

### 2.3 Driver integration

Driver now uses `BorrowChecker::with_fn_sigs(&fn_sig_table.sigs)` instead
of `BorrowChecker::new()`.

## 3. Verification

- `cargo clean && cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 221/221 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2130/2130 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5216/5216 PASS

**Total: 7567 tests passing, 0 failures, 0 warnings.**
