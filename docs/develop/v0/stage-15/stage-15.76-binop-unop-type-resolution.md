# Stage 15.76 — Binary/Unary Op Type Resolution

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.200.0 → v0.201.0
> **Process**: stage-committee-process.md v3.24 §29

## 1. Executive Summary

Stage 15.76 improves type resolution for binary and unary operations in
MIR lowering. Instead of creating fresh `Infer` types (which remain
unresolved at borrowck time), the result types are now resolved from
the operand types:

- **Comparison ops** (`==`, `!=`, `<`, `>`, `<=`, `>=`): result type is `Bool`.
- **Arithmetic ops** (`+`, `-`, `*`, `/`, `%`, `&`, `|`, `^`, `<<`, `>>`):
  result type is the lhs operand's type (same as Rust: `a + b` has type of `a`).
- **Unary ops** (`-`, `!`): result type is the inner operand's type.

This follows the same pattern as Stages 15.73 and 15.75 — avoid creating
`Infer` types at MIR lowering time that remain unresolved at borrowck time.

Per §1.0 原則 3 "显式 > 隐式": result types are explicitly resolved.
Per §16: reads only MIR data (local_decls), no HIR lookup.

**Total: 7567 tests passing, 0 failures, 0 warnings.**

## 2. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 221/221 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2130/2130 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5216/5216 PASS
