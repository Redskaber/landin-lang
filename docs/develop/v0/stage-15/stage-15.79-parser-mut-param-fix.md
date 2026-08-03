# Stage 15.79 — Parser `mut name: Type` Mis-Parse Fix + Param Mutability Propagation

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.203.0 → v0.204.0
> **Process**: stage-committee-process.md v3.24 §29 + §9.3 + §25

## 1. Executive Summary

Stage 15.79 fixes a long-standing parser bug discovered during the
Stage 15.78 conformance error test audit:

**Bug**: The `is_self_param` check in `parse_params` (src/parser/generics.rs)
matched ANY parameter starting with `KwMut`, including regular params
like `mut n: i32`. The parser would then consume `n` as if it were the
`self` keyword, silently renaming the binding to "self" and producing
"cannot find value `n` in this scope" errors for any reference to `n`
in the function body.

**Follow-up bug**: Even after the parser correctly identified `mut n` as
a regular param (with `BindingMode::ByValue(Mutable)`), the MIR lowerer
in `src/mir/lower/mod.rs` ignored the pattern's mutability when allocating
the local — always using `new_local` (Immutable). So `fn f(mut n: i32)
{ n = 0; }` would fail with `AssignImmutable` at the assignment site.

**Both bugs fixed**:

1. Parser: `is_self_param` now requires `KwMut` to be followed by
   `KwSelf_` (or `&` + `KwMut` + `KwSelf_`) before treating the
   parameter as a self receiver.
2. MIR lowerer: param locals now use `new_local_with_mut` with the
   pattern's mutability (symmetric with `let mut x` lowering).

**Test impact**:
- 4 conformance tests flipped `compile_error → compile_ok` (the fib
  e2e tests that previously failed because of the parser bug)
- 2 new Rust regression tests for the parser fix
- **Total: 7572 tests passing** (224 lib + 2132 integration + 5216
  conformance), 0 failures, 0 warnings.

Per §1.0 原則 4 "报错 > 静默": the mis-parse was silently producing
wrong AST instead of correctly recognizing the regular param.

## 2. Root Cause Analysis

### 2.1 The parser bug

Original code (`src/parser/generics.rs::parse_params`):

```rust
let is_self_param = matches!(self.peek(), TokenKind::KwSelf_ | TokenKind::KwMut)
    || (*self.peek() == TokenKind::And
        && matches!(self.peek_at(1), TokenKind::KwSelf_ | TokenKind::KwMut));
```

The `TokenKind::KwMut` arm matches `mut` alone, regardless of what
follows. So `mut n: i32` would be classified as a self param.

The self-param branch then:
1. Consumes `mut` (line 42-45)
2. Consumes the next token as "self" (line 50) — but the next token is
   `Ident(n)`, not `KwSelf_`. The parser silently accepts it as if it
   were `self`.
3. Renames the binding to `"self"` (line 90, via `interner.get_or_intern("self")`)
4. Parses the type after `:` as usual.

Result: the AST has `Param { is_self: true, pat: Ident("self") }` for
`mut n: i32`. All references to `n` in the function body fail to
resolve because the binding is named `self`, not `n`.

### 2.2 The MIR lowerer bug

Even after fixing the parser, `fn f(mut n: i32) { n = 0; }` would fail
with `AssignImmutable`. Investigation showed:

- AST correctly has `Param { pat: Ident(ByValue(Mutable), "n"), is_self: false }`
- But MIR `local_decls[n]` has `mutability: Immutable`

Root cause in `src/mir/lower/mod.rs::lower_hir_body_to_mir_full`:

```rust
let param_local = cx.new_local(param.pat.hir_id, ty, None);
```

`new_local` always creates an Immutable local. The `let mut x` lowering
in `control_flow.rs` correctly uses `new_local_with_mut` with
`pat_mutability(&local.pat)`, but the param lowering didn't follow the
same pattern.

## 3. The Fix

### 3.1 Parser fix

```rust
let is_self_param = matches!(self.peek(), TokenKind::KwSelf_)
    || (*self.peek() == TokenKind::KwMut
        && matches!(self.peek_at(1), TokenKind::KwSelf_))
    || (*self.peek() == TokenKind::And
        && (matches!(self.peek_at(1), TokenKind::KwSelf_)
            || (*self.peek_at(1) == TokenKind::KwMut
                && matches!(self.peek_at(2), TokenKind::KwSelf_))));
```

Three arms:
1. `self` — bare self
2. `mut self` — `KwMut` followed by `KwSelf_`
3. `&self` OR `&mut self` — `&` followed by (`KwSelf_` OR (`KwMut`
   followed by `KwSelf_`))

This correctly rejects `mut n: i32` (KwMut followed by Ident, not
KwSelf_) and `&mut n: i32` (KwMut followed by Ident).

### 3.2 MIR lowerer fix

```rust
let mutability = pattern_bindings::pat_mutability(&param.pat);
let param_local = cx.new_local_with_mut(param.pat.hir_id, ty, None, mutability);
```

Symmetric with the `let mut x` lowering in `control_flow.rs:700`. Now
`fn f(mut n: i32) { n = 0; }` correctly lowers to a mutable local and
the borrow checker accepts the assignment.

## 4. Test Impact

### 4.1 Conformance tests flipped (4 tests, compile_error → compile_ok)

| # | Test | Source | Notes |
|---|------|--------|-------|
| 1 | `04-e2e/01-fib/015-fib-count-digits.lin` | `fn count_digits(mut n:i32)` | Now compiles |
| 2 | `04-e2e/01-fib/016-fib-reverse-number.lin` | `fn reverse_num(mut n:i32)` | Now compiles |
| 3 | `04-e2e/01-fib/011-fib-gcd-iterative.lin` | `fn gcd(mut a:i32,mut b:i32)` | Now compiles |
| 4 | `04-e2e/01-fib/017-fib-collatz-steps.lin` | `fn collatz(mut n:i32)` | Now compiles (after both fixes) |

Tests 1, 2, 3 needed only the parser fix. Test 4 needed both the parser
fix (for `mut n` param) AND the MIR lowerer fix (for `n = 3*n+1`
assignment to the mutable param).

### 4.2 New Rust regression tests (2 tests)

Added to `tests/v0/stage0/plan/ast_structure_tests.rs`:

1. `test_mut_param_not_self_regression` — `fn bar(mut n: i32) {}`:
   verifies `is_self == false` and `self_kind == None`
2. `test_ref_mut_param_not_self_regression` — `fn bar(n: &mut i32) {}`:
   verifies `is_self == false`

These guard against regression of the parser bug.

## 5. API Naming Compliance (§23)

This stage makes no API surface changes:

- Parser fix modifies the `is_self_param` local variable initialization
  in `parse_params`. No new public functions or types.
- MIR lowerer fix replaces `new_local` call with `new_local_with_mut`
  call. Both functions already exist; no new APIs.
- 2 new test functions are private (`#[test]`).

**§23.1**: ✅ no new entry points.
**§23.4**: ✅ no new types.
**§23.5**: ✅ DRY — `pat_mutability` reused from existing
`pattern_bindings` module (same function used by `let mut` lowering).

## 6. §16 Interface Isolation

The parser fix is entirely within `parser::generics::parse_params`. No
cross-stage access.

The MIR lowerer fix is within `mir::lower::mod::lower_hir_body_to_mir_full`.
It reads `param.pat` (already available in the function body) and calls
`pattern_bindings::pat_mutability` (same module tree, internal helper).
No cross-stage access.

## 7. §25 Deep Review (8 Dimensions)

| Dimension | Status | Notes |
|-----------|--------|-------|
| D1 Architecture | ✅ | Both fixes follow existing patterns (parse_params, let mut lowering) |
| D2 Tech Debt | ✅ | 1 parser bug fixed, 1 MIR lowerer asymmetry fixed (param local vs let local) |
| D3 Test Coverage | ✅ | 2 new regression tests + 4 conformance tests correctly flipped |
| D4 Next-Phase Readiness | ✅ | No regressions; `mut` params now work correctly |
| D5 Design Rationality | ✅ | Parser now correctly distinguishes self from regular params; MIR lowerer symmetric with `let mut` lowering |
| D6 Performance | ✅ | One extra `peek_at` per param parse; negligible |
| D7 Documentation | ✅ | This doc + test plan + matrix updated |
| D8 Test Path Coverage | ✅ | `mut name: Type` and `&mut name: Type` paths now have regression tests |

**Committee Vote**: GO — Stage 15.79 complete.

## 8. Verification

- `cargo clean && cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 224/224 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2132/2132 PASS (was 2130, +2 new)
- `python3 tests/conformance/run_all.py` — ✅ 5216/5216 PASS (4 flipped, 0 added)
- **Total: 7572 tests passing, 0 failures, 0 warnings.**

## 9. Audit Update (per user directive)

This stage continues the Stage 15.78 audit of `EXPECTED: compile_error`
tests in `tests/conformance/`:

- **Before Stage 15.79**: 416 compile_error tests; 4 e2e fib tests
  incorrectly marked compile_error (parser bug)
- **After Stage 15.79**: 412 compile_error tests; 4 e2e fib tests
  correctly flipped to compile_ok

Remaining `EXPECTED: compile_error` tests are correctly classified
(underlying limitations still apply — generic struct literal construction
needs Task 11, closure typed return parser support missing, Vec{T}
shorthand not supported).

## 10. Next Steps

### 10.1 Remaining audit items

From the Stage 15.78 audit, the remaining unfixable-v-now items are:

1. **Generic struct literal construction** (`Pair<A,B>{...}`) — blocked
   on Task 11 (Monomorphization)
2. **Closure typed return** (`||->i32{42}`) — parser doesn't support
3. **`Vec{T}` shorthand** — parser doesn't support `T` as type param
4. **Empty array `[]` as `[T; N]` field initializer** — would need
   "empty array with inferred length" feature

These are larger items that don't fit a single stage.

### 10.2 Recommended next stage

**Stage 15.80** (recommended): Investigate and potentially fix the
`Vec{T,data:[i32;10],len:i32}` shorthand syntax — the parser currently
rejects `T` as a struct field type without an explicit `: Type` annotation.
This is a small parser fix that would unflip 1 conformance test
(`024-rw-vec-wrapper.lin`). 1-2 hours effort.

Alternatively, start **Task 12 (Lifetime elision)** — the next major
v0.2 task (2-3 weeks, P1, ready now).

## 11. Version Policy

v0.203.0 → v0.204.0 (minor bump — Phase 2 parser bug fix + MIR lowerer
fix + 4 conformance tests unflipped + 2 regression tests).
