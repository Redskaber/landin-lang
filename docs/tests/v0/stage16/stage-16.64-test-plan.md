# Stage 16.64 — Test Plan: Task 14 Object Safety Checking

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.250.0

## 1. Test Scope

Stage 16.64 implements object safety checking. Tests verify all 5 rules
plus safe traits and edge cases.

## 2. Test File

- `src/traits/object_safety.rs` — 10 unit tests
- All passing ✅

## 3. Unit Test Coverage (10 tests)

| # | Test | Rule | Description |
|---|------|------|-------------|
| 1 | `safe_trait_no_violations` | — | `fn bar(&self) -> i32` → 0 violations |
| 2 | `self_return_not_safe` | SelfReturn | `fn bar(&self) -> Self` → 1 violation |
| 3 | `generic_method_not_safe` | GenericMethod | `fn bar<T>(&self, x: T)` → 1 violation |
| 4 | `no_receiver_not_safe` | NoReceiver | `fn bar() -> i32` → 1 violation |
| 5 | `by_value_receiver_not_safe` | ByValueReceiver | `fn bar(self)` → 1 violation |
| 6 | `self_in_arg_not_safe` | SelfInArg | `fn bar(&self, x: Self)` → 1 violation |
| 7 | `empty_trait_safe` | — | `trait Foo {}` → 0 violations |
| 8 | `ref_mut_self_safe` | — | `fn bar(&mut self)` → 0 violations |
| 9 | `self_in_ref_return_not_safe` | SelfReturn | `fn bar(&self) -> &Self` → 1 violation |
| 10 | `multiple_violations` | — | 3 methods, 3 violations |

## 4. References

- Stage 16.64 design: `docs/develop/v0/stage-16/stage-16.64-task14-object-safety.md`
