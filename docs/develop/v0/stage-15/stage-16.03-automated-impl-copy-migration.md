# Stage 16.03 — Automated impl Copy Migration Script + Partial Migration

> **Author**: redskaber
> **Date**: 2026-08-03
> **Version**: v0.226.1 → v0.226.2
> **Process**: stage-committee-process.md v3.24 §25

## 1. Executive Summary

Stage 16.03 creates an automated `impl Copy` migration script and runs
it on the conformance test suite. The script added 393 `impl Copy` blocks
across 382 files. However, enabling sound Copy detection still causes
117 test failures (69 conformance + 48 integration) because some tests
have structs in Rust test files or complex patterns the script can't
handle automatically.

**What was done**:
1. Created `tools/migration/add_impl_copy.py` — automated migration script
2. Created `docs/tools/migration/add_impl_copy.md` — tool documentation
3. Ran the script: 393 `impl Copy` blocks added to 382 .lin files
4. Tested with sound Copy enabled: 117 failures remain (down from 247)
5. Reverted to `with_fn_sigs` for v0.2 compatibility

**Progress**: 247 → 117 failures (53% reduction). Remaining failures
need manual review (v0.3 work item).

Per user directive: "简化的设计实现需要将其完整的设计实现纳入设计
实现测试计划，不能遗漏" — migration tool + partial migration + remaining
failures documented.

## 2. Migration Script

### 2.1 Location
- `tools/migration/add_impl_copy.py`
- `docs/tools/migration/add_impl_copy.md`

### 2.2 What it does
1. Scans `.lin` files for `struct Name { ... }` or `struct Name;`
2. Checks if `impl Copy for Name {}` already exists → skip
3. Checks if `impl Drop for Name {}` exists → skip (Copy+Drop conflict)
4. If neither, adds `impl Copy for Name {}` after the struct definition

### 2.3 Results
- 393 `impl Copy` blocks added to 382 files
- 0 files had `impl Drop` conflicts (correctly skipped)
- 0 files already had `impl Copy` (all needed migration)

## 3. Sound Copy Test Results

| State | lib | integration | conformance | Total | Failures |
|-------|-----|-------------|-------------|-------|----------|
| Before migration (Stage 16.02) | 244/244 | 2096/2144 | 5025/5224 | 7365 | 247 |
| After migration (Stage 16.03) | 244/244 | 2096/2144 | 5155/5224 | 7495 | 117 |
| v0.2 compat (with_fn_sigs) | 244/244 | 2144/2144 | 5224/5224 | 7612 | 0 |

**53% failure reduction** (247 → 117). Remaining 117 failures need manual
review — they are in Rust integration test files (not .lin files) and
complex patterns the script can't handle.

## 4. Remaining Migration Work (v0.3)

### 4.1 Rust integration test files (48 failures)
The script only scans `.lin` files. Rust test files in `tests/v0/` also
need `impl Copy` added to their test structs. These need manual review
because they construct `MirBody` and `ImplInfo` directly.

### 4.2 Complex .lin patterns (69 failures)
Some `.lin` files have patterns the script missed:
- Structs defined in single-line with other items
- Structs used across multiple files
- Structs with generic parameters

### 4.3 Migration plan
1. Extend script to handle Rust test files
2. Manual review of remaining 117 failures
3. Enable `with_resolver_and_sigs` in driver
4. Remove unsound `ty_is_copy` function

## 5. Verification (reverted state)

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 244/244 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2144/2144 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 7612 tests passing, 0 failures, 0 warnings.**

## 6. Version Policy

v0.226.1 → v0.226.2 (patch bump — migration tool + partial migration,
no behavior change).
