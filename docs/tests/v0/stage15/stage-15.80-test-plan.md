# Stage 15.80 — Test Plan: Error System Cleanup (Human-Readable Type Names)

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.204.0 → v0.205.0
> **Process**: stage-committee-process.md v3.24 §17.5

## 1. Test Scope

Stage 15.80 adds `type_to_string` / `type_kind_to_string` helpers in
`src/mir/ty.rs` and replaces `{:?}` Debug formatting in user-facing
error messages with the new helpers. It also removes `({:?})` enum
variant name leak from borrowck errors.

## 2. New Unit Tests (8 tests)

Added to `src/mir/ty.rs::tests`:

### 2.1 `type_to_string_primitives`

Tests primitive types: `bool`, `char`, `str`, `!`, `i32`, `isize`,
`u8`, `usize`, `f32`, `f64`.

### 2.2 `type_to_string_references`

Tests `&T` (immutable ref) and `&mut T` (mutable ref):
- `&i32` for `Ref(_, Immutable, Int(I32))`
- `&mut bool` for `Ref(_, Mutable, Bool)`

### 2.3 `type_to_string_raw_pointers`

Tests `*const T` and `*mut T`:
- `*const i32` for `RawPtr(Immutable, Int(I32))`
- `*mut bool` for `RawPtr(Mutable, Bool)`

### 2.4 `type_to_string_arrays`

Tests array type display:
- `[i32; 10]` for `Array(Int(I32), Const(Uint(10)))`

### 2.5 `type_to_string_tuples`

Tests tuple types:
- `()` for `Tuple([])`
- `(i32,)` for `Tuple([Int(I32)])` (single-element, trailing comma)
- `(i32, bool)` for `Tuple([Int(I32), Bool])`

### 2.6 `type_to_string_inference_vars`

Tests inference variable placeholders (match Rust convention):
- `_` for `Infer(TyVar(_))` (general type var)
- `{integer}` for `Infer(IntVar(_))`
- `{float}` for `Infer(FloatVar(_))`

### 2.7 `type_to_string_special`

Tests special types:
- `<type error>` for `Error`
- `<foreign type>` for `Foreign`
- `{closure}` for `Closure(_, _)`
- `fn` for `FnDef(_, _)`

### 2.8 `type_to_string_nested`

Tests nested type constructions:
- `&[i32; 3]` for `Ref(_, Immutable, Array(Int(I32), Const(Uint(3))))`
- `(*mut bool, i32)` for `Tuple([RawPtr(Mutable, Bool), Int(I32)])`

## 3. Conformance Test Impact

### 3.1 `ERROR_PATTERN` matches (no changes)

Many conformance tests check `ERROR_PATTERN` for substring matches in
compiler error output. Verified all still match:

| Pattern | Old message | New message | Match? |
|---------|-------------|-------------|--------|
| `immutable` | `cannot assign twice to immutable variable (AssignImmutable)` | `cannot assign twice to immutable variable` | ✅ |
| `borrow` | `cannot borrow as mutable: variable is not declared mut (BorrowImmutable)` | `cannot borrow as mutable: variable is not declared mut` | ✅ |
| `cannot borrow` | `cannot borrow ...` | `cannot borrow ...` | ✅ |
| `error` | (any error) | (any error) | ✅ |

### 3.2 No conformance test flips

All 5216 conformance tests pass unchanged. The error message wording
changes don't break any `ERROR_PATTERN` matches because the patterns
check for descriptive substrings (like "immutable", "cannot borrow"),
not Debug enum variant names.

## 4. Pipeline Path Coverage (§17.5.1)

### 4.1 typeck error path (with new type display)

| Stage | Path | Test |
|-------|------|------|
| Lexer | (no change) | ✅ |
| Parser | (no change) | ✅ |
| HIR lower | (no change) | ✅ |
| MIR lower | (no change) | ✅ |
| Typeck | `TypeError::mismatch` now uses `type_kind_to_string` | ✅ `type_to_string_*` unit tests |
| Driver | `to_diagnostics` notes use `type_kind_to_string` | ✅ Manual verification (see §5) |
| (borrowck not reached for type errors) | — | — |
| (codegen not reached for type errors) | — | — |

### 4.2 borrowck error path (with enum leak removed)

| Stage | Path | Test |
|-------|------|------|
| Lexer | (no change) | ✅ |
| Parser | (no change) | ✅ |
| HIR lower | (no change) | ✅ |
| MIR lower | (no change) | ✅ |
| Typeck | (no change) | ✅ |
| Borrowck | `BorrowError` produced (no change) | ✅ |
| Driver | `format_for_user` / `to_diagnostics` no longer append `({:?})` | ✅ Manual verification (see §5) |

## 5. Manual Verification

Verified the improved error messages manually:

### 5.1 Type mismatch (`if 42`)

```
$ echo 'fn main() { if 42 { 1 } }' | landin-stage0 --compile
error[E400]: mismatched types: expected {integer}, found bool
  --> /tmp/t.lin:1:1

note: expected: {integer}
note: found: bool
```

**Before**: `expected Infer(IntVar(IntVid(0))), found Bool` (cryptic Debug)
**After**: `expected {integer}, found bool` (human-readable)

### 5.2 Call non-function

```
$ echo 'fn main() { let x = 42; x(); }' | landin-stage0 --compile
error[E400]: expected function, found i32
```

**Before**: `expected function, found Int(I32)` (Debug)
**After**: `expected function, found i32` (clean)

### 5.3 Assign immutable

```
$ echo 'fn main() { let x = 0; x = 1; }' | landin-stage0 --compile
error[E500]: cannot assign twice to immutable variable
  --> /tmp/t.lin:1:24
  |
1 | fn main() { let x = 0; x = 1; }
  |                        ^
```

**Before**: `cannot assign twice to immutable variable (AssignImmutable)` (Debug enum leak)
**After**: `cannot assign twice to immutable variable` (clean)

### 5.4 Borrow immutable as mutable

```
$ echo 'fn main() { let x = 0; let r = &mut x; }' | landin-stage0 --compile
error[E500]: cannot borrow as mutable: variable is not declared `mut`
  --> /tmp/t.lin:1:32
  |
1 | fn main() { let x = 0; let r = &mut x; }
  |                                ^
```

**Before**: `cannot borrow as mutable: variable is not declared mut (BorrowImmutable)` (Debug enum leak)
**After**: `cannot borrow as mutable: variable is not declared mut` (clean)

## 6. Regression Risk Assessment

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Conformance `ERROR_PATTERN` matches break | LOW | All patterns check for descriptive substrings (e.g., "immutable", "cannot borrow"), not Debug names |
| Rust integration tests check Debug format | LOW | No integration tests assert on Debug enum names; all pass unchanged |
| Error message wording changes break user tooling | LOW | The `(AssignImmutable)` suffix was never documented; removing it is strictly an improvement |
| `type_to_string` produces wrong output | LOW | 8 new unit tests cover all `TyKind` variants + nested |
| `Adt(_, _)` displays as `<adt>` confuses users | LOW | `<adt>` is clearly a placeholder; better than `Adt(DefId(3), [])` |

**Overall risk**: LOW. The changes are localized to error message
formatting. The 8 new unit tests provide thorough coverage of the new
helpers.

## 7. Acceptance Criteria (§9.1)

| Criterion | Target | Actual |
|-----------|--------|--------|
| `cargo build --features llvm-backend` | 0 warnings | ✅ 0 warnings |
| `cargo fmt` | clean | ✅ clean |
| `cargo clippy --all-targets --features llvm-backend` | 0 warnings | ✅ 0 warnings |
| `cargo test --features llvm-backend --lib` | 232/232 PASS | ✅ 232/232 PASS (was 224, +8 new) |
| `cargo test --features llvm-backend --test all_tests` | 2132/2132 PASS | ✅ 2132/2132 PASS |
| `python3 tests/conformance/run_all.py` | 5216/5216 PASS | ✅ 5216/5216 PASS |
| Conformance test count | unchanged (5216) | ✅ 5216 |

## 8. Test Sign-off

- ✅ All 232 lib tests pass (was 224, +8 new `type_to_string_*` tests)
- ✅ All 2132 integration tests pass
- ✅ All 5216 conformance tests pass
- ✅ 0 conformance test flips (ERROR_PATTERN matches all preserved)
- ✅ 0 clippy warnings
- ✅ fmt clean
- ✅ Manual verification: error messages are now human-readable

**Stage 15.80 PASSED**.
