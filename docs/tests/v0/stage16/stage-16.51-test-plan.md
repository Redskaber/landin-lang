# Stage 16.51 — Test Plan: Substs Propagation

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.237.0

## 1. Test Scope

Stage 16.51 propagates generic args into SubstsRef. Tests verify:
1. Generic types compile (Option<T>, Vec<T>, etc.)
2. No regressions on non-generic code
3. All conformance tests pass

## 2. Test File

No separate test file — verified via conformance tests (all 5224 pass).
The key test: `enum Option<T> { Some(T), None } fn main() -> Option<i32>`
now compiles successfully.

## 3. References

- Stage 16.51 design: `docs/develop/v0/stage-16/stage-16.51-substs-propagation.md`
- Task 11 design: `docs/develop/v0/task-11-monomorphization-design.md`
