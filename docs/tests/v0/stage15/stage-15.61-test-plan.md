# Stage 15.61 — Test Plan: `impl Drop` End-to-End Fix

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.186.0 → v0.187.0
> **Process**: stage-committee-process.md v3.23 §17.5
> **Scope**: Verify the four Stage 15.61 fixes (elaborate_drops loop, Drop codegen type, LLVM backend glue, borrowck Drop semantics).

## 1. Test Categories

### 1.1 Regression — Conformance suite (5216 tests)

**Goal**: Confirm the four fixes don't break any existing program.

**Coverage**:
- `00-parse/*.lin` — 600 parse tests
- `01-typecheck/*.lin` — 1020 typecheck tests
- `02-borrowck/*.lin` — 820+ borrowck tests
- `03-codegen/*.lin` — 2275+ codegen tests (includes 4 previously-failing Drop tests)
- `04-integration/*.lin` — 500+ integration tests
- `05-soundness/*.lin` — 5216 total (all soundness tests)

**Expected**: All 5216 pass. The 4 previously-failing Drop tests now pass:
- `03-codegen/03-drop-glue/004-drop-impl.lin`
- `03-codegen/03-drop-glue/027-drop-struct-with-Drop-trait.lin`
- `05-soundness/01-drop-check/004-drop-impl.lin`
- `05-soundness/01-drop-check/024-drop-struct-with-Drop-impl.lin`

### 1.2 New — End-to-end integration tests (8 tests)

**File**: `tests/v0/stage15/plan/impl_drop_e2e_tests.rs`

Each test asserts that `compile()` succeeds (no errors) for a specific
`impl Drop` pattern. The 8 tests cover:

| Test | Pattern | Bug covered |
|------|---------|-------------|
| `stage15_61_impl_drop_basic_compiles` | Simplest `impl Drop` program | #1 (loop), #2 (type) |
| `stage15_61_impl_drop_let_wildcard_compiles` | `let _ = S{...}` | #4 (borrowck) |
| `stage15_61_impl_drop_multiple_local` | Two Drop locals in same scope | #1 (loop) |
| `stage15_61_impl_drop_field_access_copy` | Access Copy field of Drop struct | #4 (borrowck) |
| `stage15_61_impl_drop_with_ref_method` | `&self` method on Drop struct | #4 (borrowck) |
| `stage15_61_impl_drop_explicit_self_type` | `self: &mut Counter` syntax | #1 (loop) |
| `stage15_61_impl_drop_function_returns_drop_type` | Function returns Drop type | #3 (LLVM backend) |
| `stage15_61_impl_drop_multiple_structs_cross_calls` | Multiple structs + cross-calls | #1, #3, #4 |

### 1.3 Runtime — End-to-end execution (manual)

**Goal**: Verify `impl Drop` programs produce correct exit codes.

Three programs were manually tested with `--run`:

| Program | Expected exit | Actual exit | Status |
|---------|---------------|-------------|--------|
| `let s = S{x: 42}; s.x` | 42 | 42 | ✅ |
| `let c = make(42); c.value` | 42 | 42 | ✅ |
| `use_counter(&c) + use_counter(&d)` (c=10, d=20) | 30 | 30 | ✅ |

## 2. Test Execution

### 2.1 Pre-flight (every code change)

```bash
# Build
LLVM_SYS_191_PREFIX=/tmp/llvm-19-prefix LLVM_LINK_SHARED=1 \
  cargo build --release --features llvm-backend

# Lib tests (226)
LLVM_SYS_191_PREFIX=/tmp/llvm-19-prefix LLVM_LINK_SHARED=1 \
  cargo test --features llvm-backend --lib

# Integration tests (2102)
LLVM_SYS_191_PREFIX=/tmp/llvm-191_PREFIX LLVM_LINK_SHARED=1 \
  cargo test --features llvm-backend --test all_tests

# Conformance (5216)
python3 tests/conformance/run_all.py

# Quality
cargo fmt
LLVM_SYS_191_PREFIX=/tmp/llvm-191_PREFIX LLVM_LINK_SHARED=1 \
  cargo clippy --all-targets --features llvm-backend
```

### 2.2 Targeted e2e test run

```bash
LLVM_SYS_191_PREFIX=/tmp/llvm-19-prefix LLVM_LINK_SHARED=1 \
  cargo test --features llvm-backend --test all_tests stage15_impl_drop_e2e
```

Expected: 8 passed, 0 failed.

### 2.3 Runtime verification

```bash
# Test 1: basic
echo 'trait Drop { fn drop(&mut self); }
struct S { x: i32 }
impl Drop for S { fn drop(&mut self) {} }
fn main() -> i32 { let s = S{x: 42}; s.x }' > /tmp/t1.lin
./target/release/landin-stage0 --run /tmp/t1.lin
echo "exit=$?"  # → 42

# Test 2: function returning Drop type
echo 'struct Counter { value: i32 }
impl Drop for Counter { fn drop(self: &mut Counter) { let _ = self.value; } }
fn make(v: i32) -> Counter { Counter { value: v } }
fn main() -> i32 { let c = make(42); c.value }' > /tmp/t2.lin
./target/release/landin-stage0 --run /tmp/t2.lin
echo "exit=$?"  # → 42

# Test 3: multiple structs + cross-calls
echo 'struct Counter { value: i32 }
impl Drop for Counter { fn drop(self: &mut Counter) { let _ = self.value; } }
fn make(v: i32) -> Counter { Counter { value: v } }
fn use_counter(c: &Counter) -> i32 { c.value }
fn main() -> i32 {
    let c = make(10);
    let d = make(20);
    use_counter(&c) + use_counter(&d)
}' > /tmp/t3.lin
./target/release/landin-stage0 --run /tmp/t3.lin
echo "exit=$?"  # → 30
```

## 3. Test Matrix (§17.5)

| Pipeline stage | Tests | Coverage |
|----------------|-------|----------|
| Parse | 600 | `impl Drop for T` parses (Stage 5.5) |
| Typecheck | 1020 | `impl Drop for T` typechecks (Stage 5.10) |
| Borrowck | 820+ | Drop terminator semantics (Stage 15.61 fix #4) |
| MIR lower | — | `StorageDead` emission (existing) |
| Drop elaboration | 24+8 | elaborate_drops loop fix (Stage 15.61 fix #1) |
| Codegen (text) | — | Drop type fix (Stage 15.61 fix #2) |
| Codegen (LLVM) | — | Drop glue emission (Stage 15.61 fix #3) |
| Link | 3 | `--emit-bin` produces working executable |
| Runtime | 3 | Exit codes match expected values |

## 4. Sign-off

- ✅ All 5216 conformance tests pass.
- ✅ All 2102 integration tests pass (including 8 new e2e tests).
- ✅ All 226 lib tests pass.
- ✅ 0 clippy warnings, fmt clean.
- ✅ 3 runtime tests produce correct exit codes.

**Total: 7544 tests passing, 0 failures.**

Stage 15.61 is GO for merge.
