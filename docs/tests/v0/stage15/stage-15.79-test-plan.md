# Stage 15.79 — Test Plan: Parser `mut name: Type` Mis-Parse Fix + Param Mutability Propagation

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.203.0 → v0.204.0
> **Process**: stage-committee-process.md v3.24 §17.5

## 1. Test Scope

Stage 15.79 fixes two bugs:

1. **Parser bug**: `is_self_param` check in `parse_params` mis-parsed
   `mut name: Type` as `mut self: Type`, silently renaming the binding
   to "self".
2. **MIR lowerer bug**: `lower_hir_body_to_mir_full` ignored the
   pattern's mutability when allocating param locals, always using
   `new_local` (Immutable) instead of `new_local_with_mut`.

## 2. New Regression Tests (2 tests)

Added to `tests/v0/stage0/plan/ast_structure_tests.rs`:

### 2.1 `test_mut_param_not_self_regression`

```rust
let (krate, errors) = parse("fn bar(mut n: i32) {}");
assert!(errors.is_empty());
match &krate.items[0].kind {
    ItemKind::Fn(fn_decl) => {
        let p = &fn_decl.sig.inputs[0];
        assert!(!p.is_self, "`mut n: i32` should NOT be parsed as a self param");
        assert_eq!(p.self_kind, None, "`mut n: i32` should have self_kind=None");
    }
    other => panic!("expected Fn, got {:?}", other),
}
```

Verifies the parser correctly classifies `mut n: i32` as a regular
param, not a self receiver.

### 2.2 `test_ref_mut_param_not_self_regression`

```rust
let (krate, _errors) = parse("fn bar(n: &mut i32) {}");
match &krate.items[0].kind {
    ItemKind::Fn(fn_decl) => {
        let p = &fn_decl.sig.inputs[0];
        assert!(!p.is_self, "`n: &mut i32` should NOT be parsed as a self param");
    }
    other => panic!("expected Fn, got {:?}", other),
}
```

Verifies the parser correctly classifies `n: &mut i32` (a regular ref
param with mutable borrow) as a regular param, not a self receiver.

## 3. Conformance Test Changes (4 tests flipped compile_error → compile_ok)

| # | Test | Code | Why It Now Compiles |
|---|------|------|---------------------|
| 1 | `04-e2e/01-fib/015-fib-count-digits.lin` | `fn count_digits(mut n:i32)->i32{let mut c=0;while n>0{c+=1;n/=10;}c}` | Parser fix: `mut n` correctly parsed as regular param |
| 2 | `04-e2e/01-fib/016-fib-reverse-number.lin` | `fn reverse_num(mut n:i32)->i32{let mut r=0;while n>0{r=r*10+n%10;n/=10;}r}` | Parser fix |
| 3 | `04-e2e/01-fib/011-fib-gcd-iterative.lin` | `fn gcd(mut a:i32,mut b:i32)->i32{while b!=0{let t=b;b=a%b;a=t;}a}` | Parser fix (2 mut params) |
| 4 | `04-e2e/01-fib/017-fib-collatz-steps.lin` | `fn collatz(mut n:i32)->i32{let mut s=0;while n!=1{if n%2==0{n/=2;}else{n=3*n+1;}s+=1;}s}` | Both fixes: parser (`mut n`) + MIR lowerer (`n = 3*n+1` assignment to mutable param) |

## 4. Existing Self Param Tests (must continue to pass)

The existing self param tests in `tests/v0/stage0/plan/ast_structure_tests.rs`
verify that the 4 self receiver kinds still parse correctly:

| Test | Code | Expected |
|------|------|----------|
| `test_self_kind_value_param` | `impl Foo { fn bar(self) {} }` | `is_self=true, self_kind=Value(Immutable)` |
| `test_self_kind_value_mut_param` | `impl Foo { fn bar(mut self) {} }` | `is_self=true, self_kind=Value(Mutable)` |
| `test_self_kind_ref_param` | `impl Foo { fn bar(&self) {} }` | `is_self=true, self_kind=Ref(Immutable)` |
| `test_self_kind_ref_mut_param` | `impl Foo { fn bar(&mut self) {} }` | `is_self=true, self_kind=Ref(Mutable)` |

All 4 still pass — the parser fix preserves the existing self receiver
parsing.

## 5. Pipeline Path Coverage (§17.5.1)

### 5.1 `mut name: Type` path (new correct behavior)

| Stage | Path | Test |
|-------|------|------|
| Lexer | `KwMut`, `Ident`, `Colon`, `Ident` tokens | ✅ lexer_tests |
| Parser | `Param { is_self: false, pat: Ident(ByValue(Mutable), name) }` | ✅ `test_mut_param_not_self_regression` |
| HIR lower | `HirParam { self_kind: None, pat: Ident(Mutable) }` | ✅ hir_lower_tests |
| MIR lower | `local_decls[n] = { mutability: Mutable }` | ✅ (new fix) — verified by `017-fib-collatz-steps.lin` |
| Typeck | (no change) | ✅ |
| Borrowck | Accepts `n = ...` assignment | ✅ (verified by conformance tests) |
| Codegen | (no change) | ✅ |

### 5.2 `mut self` path (preserved behavior)

| Stage | Path | Test |
|-------|------|------|
| Lexer | `KwMut`, `KwSelf_` tokens | ✅ lexer_tests |
| Parser | `Param { is_self: true, self_kind: Value(Mutable), pat: Ident("self") }` | ✅ `test_self_kind_value_mut_param` |
| HIR lower | (no change) | ✅ |
| MIR lower | (no change — uses `resolve_self_param_type` for self params) | ✅ |
| Typeck | (no change) | ✅ |
| Borrowck | (no change) | ✅ |
| Codegen | (no change) | ✅ |

## 6. Regression Risk Assessment

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Self param parsing regresses | LOW | All 4 existing self param tests still pass |
| Other param forms break | LOW | All 5216 conformance tests pass; only 4 flipped (intentional) |
| Codegen regressions | LOW | No codegen changes |
| Borrowck regressions | LOW | No borrowck changes (mutability was already in MIR; lowerer now correctly sets it) |

**Overall risk**: LOW. The parser fix narrows the `is_self_param` check
(more specific), and the MIR lowerer fix uses an existing helper
(`pat_mutability`) already used by the `let mut` lowering.

## 7. Acceptance Criteria (§9.1)

| Criterion | Target | Actual |
|-----------|--------|--------|
| `cargo build --features llvm-backend` | 0 warnings | ✅ 0 warnings |
| `cargo fmt` | clean | ✅ clean |
| `cargo clippy --all-targets --features llvm-backend` | 0 warnings | ✅ 0 warnings |
| `cargo test --features llvm-backend --lib` | 224/224 PASS | ✅ 224/224 PASS |
| `cargo test --features llvm-backend --test all_tests` | 2132/2132 PASS | ✅ 2132/2132 PASS (was 2130, +2 new) |
| `python3 tests/conformance/run_all.py` | 5216/5216 PASS | ✅ 5216/5216 PASS (4 flipped, 0 added) |
| Conformance test count | unchanged (5216) | ✅ 5216 |

## 8. Test Sign-off

- ✅ All 224 lib tests pass
- ✅ All 2132 integration tests pass (was 2130, +2 new regression tests)
- ✅ All 5216 conformance tests pass
- ✅ 4 conformance tests correctly flipped (parser bug fix)
- ✅ 0 clippy warnings
- ✅ fmt clean

**Stage 15.79 PASSED**.
