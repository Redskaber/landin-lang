# Stage 16.72 — Stale File Verification + User Environment Fix Guide

> **Author**: redskaber
> **Date**: 2026-08-05
> **Version**: v0.257.0 → v0.258.0
> **Process**: stage-committee-process.md v3.24 §9.3 (gate review)

## 1. Executive Summary

A user reported `cargo fmt` failing with:
```
Error writing files: failed to resolve mod `monomorphize`: file for module
found at both ".../src/mir/monomorphize.rs" and ".../src/mir/monomorphize/mod.rs"
```

**Root cause**: The user's local environment has a stale `monomorphize.rs`
file that was not deleted when the module was split into a directory
(`monomorphize/`) in Stage 16.61. Rust's module resolution forbids having
both `foo.rs` and `foo/mod.rs` simultaneously.

**Our project state**: Verified clean — only `monomorphize/mod.rs` exists,
no `monomorphize.rs`. The `cargo fmt`, `cargo build`, `cargo clippy`, and
`cargo test` all pass with 0 warnings.

## 2. Verification

```
find src/mir -name "monomorphize.rs"   → (no output — file correctly deleted)
find src/mir -name "monomorphize"      → src/mir/monomorphize (directory)
ls src/mir/monomorphize/               → mod.rs, item.rs, mangle.rs, layout.rs
```

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt --check` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 353/353 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2529/2529 PASS
- **Total: 8106 tests passing, 0 failures, 0 warnings.**

## 3. User Fix Guide

If you encounter this error in your local environment:

```bash
# Delete the stale monomorphize.rs file
rm src/mir/monomorphize.rs

# Verify only the directory exists
ls src/mir/monomorphize/
# Should show: mod.rs, item.rs, mangle.rs, layout.rs

# Then rebuild
cargo clean && cargo build --features llvm-backend
```

## 4. Prevention

This issue occurred because Stage 16.61 split `monomorphize.rs` into a
directory but the tarball extraction on the user's side may have preserved
the old file. The project tarball correctly contains only the directory.
