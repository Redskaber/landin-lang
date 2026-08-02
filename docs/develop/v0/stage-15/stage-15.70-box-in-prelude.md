# Stage 15.70 — Box<T> in Prelude (Task 20)

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.194.0 → v0.195.0
> **Process**: stage-committee-process.md v3.24 §29
> **v0.2 Phase 4 Task 20**: `Box<T>` in prelude — 2 days, P2, needs Task 13 (DONE)

## 1. Executive Summary

Stage 15.70 registers `Box` as a builtin prelude type so that `Box<i32>` type
annotations resolve without a user-defined `struct Box`. This is the first step
toward full `Box<T>` support (heap allocation + Deref + Drop).

**Key results**:
- `Box` is registered in `build_module_tree` as a builtin type with sentinel
  `DefId(u32::MAX - 1)`.
- User-defined `struct Box` shadows the builtin (no conflict error).
- `Box` type annotations (`let x: Box<i32>`) now resolve successfully.
- Changed `resolve_crate` signature from `&Rodeo` to `&mut Rodeo` (needed for
  `interner.get_or_intern("Box")`).
- Updated 7 test files to pass `&mut interner` to `resolve_crate`.
- All 7567 tests pass (221 lib + 2130 integration + 5216 conformance).

## 2. What Was Done

### 2.1 Register Box in `build_module_tree`

In `src/resolve/module_build.rs`, `build_module_tree` now pre-registers `Box`
as a builtin struct type with `DefId(u32::MAX - 1)` (sentinel for builtin Box).
This makes `Box` available in every Landin program without `use` or `struct`
declaration.

### 2.2 User-defined Box shadows builtin

When a user writes `struct Box<T> { ... }`, the registration code detects the
conflict with the builtin `Box` (DefId(u32::MAX - 1)) and allows the
user-defined version to shadow it. This preserves backward compatibility with
existing tests that define their own `Box`.

### 2.3 `resolve_crate` signature change

`resolve_crate` now takes `&mut Rodeo` (was `&Rodeo`) because `build_module_tree`
needs to intern the "Box" string. Updated:
- `src/resolve/resolver.rs` — `resolve_crate` + `resolve` signatures.
- `src/resolve/module_build.rs` — `build_module_tree` signature.
- `src/driver.rs` — call site.
- 7 test files — call sites.

## 3. Verification

- `cargo clean && cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 221/221 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2130/2130 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5216/5216 PASS

**Total: 7567 tests passing, 0 failures, 0 warnings.**

## 4. Limitations (Deferred to v0.3)

This stage registers `Box` as a known type name. Full `Box<T>` support requires:
- **Monomorphization** (Task 11) — to instantiate `Box<i32>` with concrete types.
- **Heap allocation codegen** — `malloc` for `Box::new`, `free` in drop glue.
- **Deref coercion** — `*box_val` loads the pointer, then the value.
- **Generic type parameters** — `Box<T>` needs generic type parameter support.

These are deferred to v0.3 (blocked on Task 3 TraitResolver keys + Task 11
Monomorphization).

## 5. Committee Vote: GO

**Decision**: Stage 15.70 is **COMPLETE**. `Box` is registered as a prelude
type. Full `Box<T>` support deferred to v0.3 (needs monomorphization).
