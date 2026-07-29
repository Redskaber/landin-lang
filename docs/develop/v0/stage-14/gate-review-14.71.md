# Stage 14.71 — Gate Review

> **Author**: redskaber
> **Date**: 2026-07-29
> **Version**: v0.86.0 → v0.87.0
> **Process**: stage-committee-process.md v3.22 §25 (D8 review)

## 1. Stage Summary

Stage 14.71 created a debug/testing tool (`tools/debug/landin_debug.py`) and
fixed a match wildcard regression discovered by the tool.

## 2. New Feature: Debug Tool

Created a Python-based debug tool with 5 commands:
- `trace` — Trace compilation pipeline (Lexer → Parser → IR → Execute)
- `mir` — Dump MIR structure (function list)
- `test-runner` — Run all run_ok tests with pass/fail reporting
- `diff` — Compare test output with EXPECTED_STDOUT
- `stages` — Show which compilation stages pass/fail

The tool supports `EXPECTED_EXIT_CODE` and provides line-by-line diffs.

## 3. Bug Fixed: Match wildcard regression

**Discovery**: The test-runner found that `e2e-runok-011-match.lin` was failing.
`classify(5)` returned 1 instead of 10.

**Root cause**: Stage 14.67's otherwise-block rewrite reset
`cx.current_block` to `fallthrough_block` after `lower_expr_to_operand`,
orphaning overflow-check blocks and skipping the result assignment.

**Fix**: Don't reset `cx.current_block`; terminate the current (last) block.

## 4. Verification

- `cargo clean && cargo build --features llvm-backend` → ✅
- `cargo fmt` → ✅ (no changes)
- `cargo clippy --all-targets --features llvm-backend` → ✅ (0 warnings)
- `cargo test --features llvm-backend` → ✅ (1951 passed, 0 failed)
- Debug tool test-runner: 128/129 pass (1 known: self-by-value chain)
