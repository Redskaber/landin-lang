# Stage 15.75 — Deref Expression Type Resolution

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.199.0 → v0.200.0
> **Process**: stage-committee-process.md v3.24 §29

## 1. Executive Summary

Stage 15.75 improves `lower_deref_expr` to resolve the dereference result
type from the inner local's type, instead of creating a fresh `Infer` type.
If the inner is `&T` or `&mut T`, the result type is `T`. If it's `*const T`
or `*mut T`, the result type is `T`. Otherwise, falls back to `fresh_infer_ty`.

This is the same pattern as Stage 15.73 (let binding type propagation) —
avoid creating `Infer` types at MIR lowering time that remain unresolved at
borrowck time (writeback runs after borrowck).

Per §1.0 原則 3 "显式 > 隐式": the deref result type is explicitly resolved
from the reference type, not left as an implicit inference variable.
Per §16: reads only MIR data (local_decls), no HIR lookup.

**Total: 7567 tests passing, 0 failures, 0 warnings.**

## 2. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 221/221 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2130/2130 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5216/5216 PASS

**v0.200.0 milestone: 200 versions completed!**
