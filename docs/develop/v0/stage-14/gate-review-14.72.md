# Stage 14.72 — Gate Review

> **Author**: redskaber
> **Date**: 2026-07-29
> **Version**: v0.87.0 → v0.88.0
> **Process**: stage-committee-process.md v3.22 §25 (D8 review)

## 1. Stage Summary

Stage 14.72 fixes impl method name collisions that caused segfaults in
self-by-value method chains. This achieves **100% run_ok test pass rate**!

## 2. Bug Fixed

### Bug: Impl method name collisions caused segfault

**Discovery**: The debug tool test-runner found `e2e-runok-064-nested-struct-chain.lin`
segfaulted. The `stages` command confirmed all compilation stages passed but
execution crashed.

**Root cause**: HIR lowering stores impl methods as `HirItem::Fn` owners (not
nested in `HirItem::Impl`). `body_metas` looked for `HirItem::Impl` to generate
type-qualified names but never found them. All same-named methods resolved to
`landin_<method>` (no type prefix) → duplicate function definitions.

**Fix** (`src/driver.rs`):
1. Register impl methods in `fn_name_by_def_id` with type-qualified names.
2. Use `fn_name_by_def_id` for `body_metas` name resolution.

## 3. Verification

- `cargo clean && cargo build --features llvm-backend` → ✅
- `cargo fmt` → ✅ (no changes)
- `cargo clippy --all-targets --features llvm-backend` → ✅ (0 warnings)
- `cargo test --features llvm-backend` → ✅ (1951 passed, 0 failed)
- Debug tool test-runner: **129/129 pass (100%!)** 🎉
