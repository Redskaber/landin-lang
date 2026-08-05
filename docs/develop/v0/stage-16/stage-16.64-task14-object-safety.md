# Stage 16.64 — Task 14 Phase 1: Object Safety Checking

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.249.0 → v0.250.0
> **Process**: stage-committee-process.md v3.24 §13.4 + §23

## 1. Executive Summary

Stage 16.64 implements Task 14 Phase 1 — object safety checking. A trait is
object-safe if `dyn Trait` can be used for it. This module checks 5 rules
per Rust RFC #255:

1. **SelfReturn** — method returns `Self` type
2. **SelfInArg** — method has `Self` in an argument type
3. **GenericMethod** — method has generic type parameters
4. **NoReceiver** — method has no `self`/`&self`/`&mut self` receiver
5. **ByValueReceiver** — method takes `self` by value (not `&self`/`&mut self`)

**What was implemented**:

1. **`src/traits/object_safety.rs`** — new module with:
   - `ObjectSafetyViolation` enum (5 variants)
   - `check_trait_object_safety(trait_def) -> Vec<ObjectSafetyViolation>`
   - `ty_contains_self(ty) -> bool` — recursive Self detector
   - `error_message(trait_name, interner)` — human-readable error formatting
   - 10 unit tests covering all 5 rules + safe traits + empty trait + &mut self + multiple violations

2. **Re-exports** in `src/traits/mod.rs`:
   - `pub use object_safety::{check_trait_object_safety, ObjectSafetyViolation}`

**Key result**: `trait Foo { fn bar(&self) -> Self; }` is correctly detected
as not object-safe (SelfReturn violation). `trait Foo { fn bar(&self) -> i32; }`
is correctly detected as object-safe (no violations).

**Test results**: 8091 tests passing (353 lib + 2514 integration + 5224
conformance), 0 failures, 0 warnings. +10 new unit tests.

## 2. Object Safety Rules

| Rule | Example | Safe? |
|------|---------|-------|
| `&self` receiver, no Self | `fn bar(&self) -> i32` | ✅ |
| `&mut self` receiver | `fn bar(&mut self)` | ✅ |
| Empty trait | `trait Foo {}` | ✅ |
| Self in return | `fn bar(&self) -> Self` | ❌ |
| Self in argument | `fn bar(&self, x: Self)` | ❌ |
| Generic method | `fn bar<T>(&self, x: T)` | ❌ |
| No receiver | `fn bar() -> i32` | ❌ |
| By-value receiver | `fn bar(self)` | ❌ |
| Self via Ref | `fn bar(&self) -> &Self` | ❌ |

## 3. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt --check` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 353/353 PASS (+10 new)
- `cargo test --features llvm-backend --test all_tests` — ✅ 2514/2514 PASS
- **Total: 8091 tests passing, 0 failures, 0 warnings.**
