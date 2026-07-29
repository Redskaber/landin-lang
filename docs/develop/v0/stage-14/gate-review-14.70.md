# Stage 14.70 — Gate Review

> **Author**: redskaber
> **Date**: 2026-07-29
> **Version**: v0.85.0 → v0.86.0
> **Process**: stage-committee-process.md v3.22 §25 (D8 review)

## 1. Stage Summary

Stage 14.70 fixes the fat pointer `{ptr, i64}` ABI corruption that caused
string comparison to fail across function boundaries. This was the root cause
of the "known limitation" from Stage 14.69.

## 2. Bug Fixed

### Bug: Fat pointer len field corrupted across function calls

**Symptom**: `classify_name("Bob")` returned 0 instead of 2. The `i64` length
field of the fat pointer was corrupted on the 2nd+ calls to a function
receiving `&str` parameters.

**Root cause**: `LLVMSysEmitter::interpret_adhoc` parses integer literals as
`i32` constants. When `emit_insertvalue` inserts this `i32` value into an `i64`
field (the fat pointer's `len` field), LLVM stores only 4 bytes (`movl`) instead
of 8 bytes (`movq`). The upper 4 bytes remain as stack garbage.

**Fix** (`src/codegen/llvm/mod.rs`): In `emit_insertvalue`, coerce `val_v` to
the struct field's type before inserting. Uses `LLVMGetStructElementTypes` to
get the field type, then `LLVMBuildIntCast2` for integer width mismatches.

## 3. Verification

- `cargo clean && cargo build --features llvm-backend` → ✅
- `cargo fmt` → ✅ (no changes)
- `cargo clippy --all-targets --features llvm-backend` → ✅ (0 warnings)
- `cargo test --features llvm-backend` → ✅ (1951 passed, 0 failed, 2 ignored)
- Conformance tests: 5155 (was 5154, +1 new run_ok)
- Pipeline coverage: 99.7% (692 paths, 690 verified)
- String comparison now works across function boundaries ✅

## 4. Stage Outcome

**Stage 14.70 PASSED** — fat pointer ABI fix. The "known limitation" from
Stage 14.69 (cross-function string comparison) is now resolved.
