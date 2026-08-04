# Stage 16.42 — Clean Up `#[allow(unused_imports)]` in Codegen Shared Modules

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.234.1 → v0.234.2
> **Process**: stage-committee-process.md v3.24 §1.0 原則 5 "去除兼容思维"

## 1. Executive Summary

Stage 16.42 removes all `#[allow(unused_imports)]` annotations from the
codegen shared modules and fixes the underlying unused imports. This
eliminates the last "code smell" annotations in the codegen module.

**What was removed**:
- `#![allow(unused_imports)]` + `#[allow(unused_imports)]` from `statement.rs`
- `#![allow(unused_imports)]` + `#[allow(unused_imports)]` from `operand.rs`
- `#![allow(unused_imports)]` + `#[allow(unused_imports)]` from `rvalue.rs`
- `#![allow(unused_imports)]` + `#[allow(unused_imports)]` from `terminator.rs`

**What was fixed** (underlying unused imports removed):
- `statement.rs`: Removed `use crate::mir::body::*` and `use crate::mir::ty::ConstVal`
- `operand.rs`: Removed unused mir_translation imports + `use crate::mir::body::*`
- `rvalue.rs`: Removed unused mir_translation imports + `use crate::mir::body::*` + `use crate::mir::ty::ConstVal`
- `terminator.rs`: Removed unused mir_translation imports + `use crate::mir::body::*` + duplicate `TerminatorKind` import

**Test results**: 7856 tests passing, 0 failures, 0 warnings. No behavior change.

## 2. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 244/244 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2388/2388 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 7856 tests passing, 0 failures, 0 warnings.**

## 3. Version Policy

v0.234.1 → v0.234.2 (patch bump — import cleanup, no API change, no behavior change.)
