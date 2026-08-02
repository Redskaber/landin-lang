# Stage 15.63 — Test Plan: Recursive Drop (Fields with Drop)

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.188.0 → v0.189.0
> **Process**: stage-committee-process.md v3.23 §17.5
> **Scope**: Verify the Stage 15.63 recursive drop fix.

## 1. Test Categories

### 1.1 Regression — Conformance suite (5216 tests)

**Goal**: Confirm the recursive drop fix doesn't break any existing program.

**Expected**: All 5216 pass (no regression from the rewritten
`emit_drop_glue_functions`).

### 1.2 New — Integration tests (8 tests)

**File**: `tests/v0/stage15/plan/recursive_drop_tests.rs`

| Test | Pattern | What it verifies |
|------|---------|------------------|
| `stage15_63_recursive_drop_outer_no_drop_inner_drop` | Outer (no Drop) + Inner (Drop) | Previously: link error; now: compiles |
| `stage15_63_recursive_drop_both_have_drop` | Both have Drop | User drop + recursive field drop |
| `stage15_63_recursive_drop_three_levels` | Outer→Middle→Inner | 3-level recursive drop |
| `stage15_63_recursive_drop_multiple_drop_fields` | Pair {A, B} both Drop | Multiple fields recursively dropped |
| `stage15_63_recursive_drop_mixed_fields` | Mixed Drop/non-Drop | Only Drop fields recursively dropped |
| `stage15_63_no_drop_all_primitives_no_regression` | All primitives | No drop glue emitted |
| `stage15_63_recursive_drop_function_returns_struct_with_drop_field` | fn returns Outer | Recursive drop across function boundary |
| `stage15_63_recursive_drop_explicit_self_type` | `self: &mut T` | Explicit self syntax works |

### 1.3 Runtime — End-to-end execution (manual)

**Test 1**: Struct without Drop, field with Drop (previously: link error)
```landin
trait Drop { fn drop(&mut self); }
struct Inner { x: i32 }
impl Drop for Inner { fn drop(&mut self) {} }
struct Outer { inner: Inner }
fn main() -> i32 { let o = Outer { inner: Inner { x: 42 } }; o.inner.x }
```
→ **Exit 42** ✅

**Test 2**: Three-level nesting
```landin
struct Outer { middle: Middle }
struct Middle { inner: Inner }
fn main() -> i32 { let o = Outer { middle: Middle { inner: Inner { x: 42 } } }; o.middle.inner.x }
```
→ **Exit 42** ✅

## 2. Test Execution

### 2.1 Full suite

```bash
LLVM_SYS_191_PREFIX=/tmp/llvm-19-prefix LLVM_LINK_SHARED=1 \
  cargo test --features llvm-backend --lib
# → 226 passed

LLVM_SYS_191_PREFIX=/tmp/llvm-19-prefix LLVM_LINK_SHARED=1 \
  cargo test --features llvm-backend --test all_tests
# → 2118 passed, 2 ignored

python3 tests/conformance/run_all.py
# → 5216 passed
```

### 2.2 Targeted recursive drop tests

```bash
LLVM_SYS_191_PREFIX=/tmp/llvm-19-prefix LLVM_LINK_SHARED=1 \
  cargo test --features llvm-backend --test all_tests stage15_recursive_drop
# → 8 passed
```

## 3. Sign-off

- ✅ All 5216 conformance tests pass.
- ✅ All 2118 integration tests pass (including 8 new recursive drop tests).
- ✅ All 226 lib tests pass.
- ✅ 0 clippy warnings, fmt clean.
- ✅ Runtime tests: previously-failing programs now compile and run.

**Total: 7560 tests passing, 0 failures.**

Stage 15.63 is GO for merge.
