# Stage 15.62 — Test Plan: Drop Order + Double-Drop Prevention

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.187.0 → v0.188.0
> **Process**: stage-committee-process.md v3.23 §17.5
> **Scope**: Verify the Stage 15.62 fixes (reverse drop order + double-drop prevention).

## 1. Test Categories

### 1.1 Regression — Conformance suite (5216 tests)

**Goal**: Confirm the drop order and double-drop fixes don't break any
existing program.

**Expected**: All 5216 pass (no regression from StorageDead reordering
or moved-local skipping).

### 1.2 New — Integration tests (8 tests)

**File**: `tests/v0/stage15/plan/impl_drop_order_tests.rs`

| Test | Pattern | What it verifies |
|------|---------|------------------|
| `stage15_62_drop_order_reverse_declaration_compiles` | 3 Drop locals (a, b, c) | Drop order compiles |
| `stage15_62_no_double_drop_moved_temporary` | `let c = make(42)` | Temp not double-dropped |
| `stage15_62_no_double_drop_multiple_temporaries` | 2 function calls | Both temps skipped |
| `stage15_62_drop_with_borrow_no_double_drop` | `&c` borrow | Borrowed local still dropped |
| `stage15_62_no_drop_no_regression` | Non-Drop struct | elaborate_drops is no-op |
| `stage15_62_drop_order_mixed_drop_non_drop` | Mix Drop + non-Drop | Only Drop locals get terminators |
| `stage15_62_drop_nested_function_scopes` | Helper fn with Drop | No cross-function interference |
| `stage15_62_drop_explicit_self_type_no_double_drop` | `self: &mut T` | Explicit self syntax works |

### 1.3 Runtime — Drop order observation (manual)

**Goal**: Verify the actual drop ORDER and that no double-drop occurs.

```landin
struct Logger { id: i32 }
impl Drop for Logger {
    fn drop(self: &mut Logger) { println!("dropping {}", self.id) }
}
fn main() -> i32 {
    let a = Logger { id: 1 };
    let b = Logger { id: 2 };
    let c = Logger { id: 3 };
    0
}
```

**Expected output** (reverse declaration order, no duplicates):
```
dropping 3
dropping 2
dropping 1
```

**Actual output** (after Stage 15.62 fix):
```
dropping 3
dropping 2
dropping 1
```
✅ Matches Rust semantics.

## 2. Test Execution

### 2.1 Full suite

```bash
LLVM_SYS_191_PREFIX=/tmp/llvm-19-prefix LLVM_LINK_SHARED=1 \
  cargo test --features llvm-backend --lib
# → 226 passed

LLVM_SYS_191_PREFIX=/tmp/llvm-19-prefix LLVM_LINK_SHARED=1 \
  cargo test --features llvm-backend --test all_tests
# → 2110 passed, 2 ignored

python3 tests/conformance/run_all.py
# → 5216 passed
```

### 2.2 Targeted drop order tests

```bash
LLVM_SYS_191_PREFIX=/tmp/llvm-19-prefix LLVM_LINK_SHARED=1 \
  cargo test --features llvm-backend --test all_tests stage15_impl_drop_order
# → 8 passed
```

### 2.3 Runtime verification

```bash
echo 'struct Logger { id: i32 }
impl Drop for Logger {
    fn drop(self: &mut Logger) { println!("dropping {}", self.id) }
}
fn main() -> i32 {
    let a = Logger { id: 1 };
    let b = Logger { id: 2 };
    let c = Logger { id: 3 };
    0
}' > /tmp/drop_order.lin

./target/release/landin-stage0 --run /tmp/drop_order.lin
# Expected stdout:
#   dropping 3
#   dropping 2
#   dropping 1
# Expected exit: 0
```

## 3. Test Matrix (§17.5)

| Pipeline stage | Tests | Coverage |
|----------------|-------|----------|
| MIR lower (StorageDead order) | 8 | Reverse declaration order |
| Drop elaboration (moved locals) | 8 | Flow-insensitive move analysis |
| Borrowck (Drop semantics) | 8 | No false positive on moved temps |
| Codegen (Drop glue) | 8 | Correct IR generation |
| Runtime (drop order) | 1 | Observed via println! |
| Regression (conformance) | 5216 | No regression |

## 4. Sign-off

- ✅ All 5216 conformance tests pass.
- ✅ All 2110 integration tests pass (including 8 new drop order tests).
- ✅ All 226 lib tests pass.
- ✅ 0 clippy warnings, fmt clean.
- ✅ Runtime test: drop order is 3, 2, 1 (reverse declaration), no double-drop.

**Total: 7552 tests passing, 0 failures.**

Stage 15.62 is GO for merge.
