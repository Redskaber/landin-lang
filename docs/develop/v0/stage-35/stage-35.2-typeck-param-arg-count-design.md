# Stage 35.2 (v0.23) — TD-TYPECK-PARAM-ARG-COUNT Design

> **Author**: redskaber (PM-A + ARCH-A + DEV-A)
> **Date**: 2026-09-01
> **Version**: v0.575.0 (target)
> **Process**: stage-committee-process.md v7.5 §13.1 + §14.8
> **Complexity**: L2 (~80 LOC code + ~280 LOC tests + ~150 LOC docs)

## 1. Executive Summary

TD-TYPECK-PARAM-ARG-COUNT (P3, documented since Stage 32.3) is resolved.
The bug: typeck does not validate arg count for trait method calls when
the trait method has **no body** (declaration only).

**Root cause**: `populate_trait_default_fn_sigs` in
`src/driver/driver_codegen_prep.rs:412` skips trait methods without bodies:
```rust
if f.body.is_none() {
    continue; // No body — no fn_sig needed (it's just a declaration).
}
```
This means trait declaration methods (like `fn f(&self, a: i32, b: i32) -> i32;`)
are **not** registered in `fn_sig_table`. When typeck's `check_terminator`
tries to look up the method's sig via `fn_sigs.get(def_id)` for arg-count
validation, it returns `None` → check silently skipped.

## 2. Bug Confirmation (runtime evidence)

Verified via `examples/test_arg_count.rs`:

| Case | Source | Expected | Actual (v0.574.0) | Status |
|------|--------|----------|-------------------|--------|
| 1a | `trait T { fn f(&self, a: i32, b: i32) -> i32; } ... self.x.f(1)` | ERROR (1 arg, expected 2) | 0 errors | ❌ Silent |
| 1b | Same but with default body `{ 0 }` | ERROR | 1 error ✓ | ✅ |
| 2 | Concrete impl `impl T for S` | ERROR | 1 error ✓ | ✅ |
| 3 | Param(N) receiver, correct arg count | OK | 0 errors | ✅ |

Case 1a is the silent bug — trait declaration methods (without body) skip
arg-count validation entirely.

## 3. Rust Reference Design Alignment

Per [Rust Reference §Expressions — Method-call](https://doc.rust-lang.org/reference/expressions.html#method-call-expressions):
> The receiver type must implement the method. The number of arguments
> must match the method's signature.

Per rustc: the `check_arg_count` function
(`rustc_hir_typeck/src/check/call.rs`) reports E0061 ("this function takes
N arguments but M were provided") regardless of whether the method has a
body — it works off the trait declaration's signature.

**Rust philosophy applied**:
- §1.0 原則 4 (报错 > 静默): emit error instead of silently accepting wrong arg count.
- §1.0 原則 6 (通解 > 特解): one `populate_trait_decl_fn_sigs` for all trait methods
  (with or without body).
- §1.0 原則 10 (唯一可信数据源): the trait declaration's signature is the single
  source of truth for the expected arg count.
- §12 (最优 > 最小): root-cause fix = register trait decl methods in fn_sig_table
  so typeck's existing check_terminator can validate them.

## 4. Design

### 4.1 New Function — `populate_trait_decl_fn_sigs`

Add a new function in `src/driver/driver_codegen_prep.rs` that walks all
trait declarations and registers their methods (with or without body) in
`fn_sig_table`. For methods without body, the self type is `Error` (since
no impl exists to provide it) — but the **input count** is correctly
captured so typeck can validate arg count.

```rust
/// Stage 35.2 (v0.23 — TD-TYPECK-PARAM-ARG-COUNT): Build fn_sig_table
/// entries for ALL trait declaration methods (with or without body).
///
/// Previously, `populate_trait_default_fn_sigs` only registered methods
/// with a default body. Trait declaration methods without body (e.g.,
/// `trait T { fn f(&self, a: i32, b: i32) -> i32; }`) were NOT registered
/// → typeck's check_terminator couldn't look up their sig → silently
/// accepted wrong arg count at call sites.
///
/// This function ensures every trait method declared in source has a
/// fn_sig_table entry, so typeck can validate arg count uniformly.
///
/// Per §1.0 原則 4 (报错 > 静默): fix the silent skip.
/// Per §1.0 原則 6 (通解 > 特解): one function handles all trait methods.
/// Per §1.0 原則 10 (唯一可信数据源): trait decl sig is the source of truth.
pub(super) fn populate_trait_decl_fn_sigs(
    hir: &HirCrate,
    interner: &lasso::Rodeo,
    fn_sig_table: &mut crate::typeck::FnSigTable,
) {
    for (_, owner) in &hir.owners {
        if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Trait(t)) = owner {
            for trait_item in &t.items {
                if let crate::hir::HirTraitItem::Fn(f) = trait_item {
                    let method_def_id = f.hir_id.owner;
                    if fn_sig_table.sigs.contains_key(&method_def_id) {
                        continue; // Already registered (e.g., via populate_trait_default_fn_sigs).
                    }
                    // Use Error for self_ty since no impl exists. Typeck only
                    // needs the input count for arg-count validation.
                    // For non-self params, lower the declared type normally.
                    let inputs: Vec<crate::mir::ty::Ty> = f
                        .sig
                        .inputs
                        .iter()
                        .map(|p| {
                            if p.self_kind.is_some() {
                                // Self placeholder — use Error (will trigger
                                // type mismatch error if typeck tries to unify
                                // arg type with self type, which is desired).
                                crate::mir::ty::Ty::new(
                                    crate::mir::ty::TyKind::Error,
                                    p.span,
                                )
                            } else if let Some(ty) = &p.ty {
                                crate::mir::lower::lower_hir_ty_to_mir_ty(ty)
                            } else {
                                crate::mir::ty::Ty::new(
                                    crate::mir::ty::TyKind::Error,
                                    p.span,
                                )
                            }
                        })
                        .collect();
                    let output = match &f.sig.output {
                        HirFnRetTy::Default(_) => crate::mir::ty::Ty::new(
                            crate::mir::ty::TyKind::Tuple(Vec::new()),
                            f.span,
                        ),
                        HirFnRetTy::Ty(t) => {
                            crate::mir::lower::lower_hir_ty_to_mir_ty(t)
                        }
                    };
                    fn_sig_table.sigs.insert(
                        method_def_id,
                        crate::mir::ty::Sig {
                            inputs,
                            output: Box::new(output),
                            abi: f.sig.abi,
                            is_unsafe: f.sig.is_unsafe,
                        },
                    );
                }
            }
        }
    }
}
```

### 4.2 Wire-up in driver

Call `populate_trait_decl_fn_sigs` AFTER `populate_trait_default_fn_sigs`
(so default-body methods keep their proper self_ty from the impl, not the
Error placeholder). Place the call in `compile_inner` next to the existing
`populate_trait_default_fn_sigs` call.

### 4.3 Why Not Modify `populate_trait_default_fn_sigs` Directly?

Per §1.0 原則 6 (通解 > 特解): The existing function uses impl's self_ty
for specialization (Bug Y1 fix Stage 14.97). Removing the `if f.body.is_none()
{ continue; }` would change its semantics for body methods too —
potentially regressing Case 1b. A new function avoids touching working
code. Per §12 (最优 > 最小): focused fix.

### 4.4 Why Not Validate Arg Count in MIR Lower?

Per §1.0 原則 10 (唯一可信数据源): typeck already has the infrastructure
(`check_terminator` Call handler) — adding the check in MIR lower would
duplicate logic. Per §16 (管道流): type errors belong in typeck, not MIR
lower. Per §11 (接口隔离): MIR lower doesn't read fn_sigs (only codegen
and typeck do).

### 4.5 Self Type as Error — Why?

For trait decl methods without body, there's no impl to provide the self
type. Using `Error` is the honest representation. If typeck tries to
unify the receiver arg with `Error`, it will trigger a "type mismatch"
error — which is desired (the call site's receiver type doesn't match
the trait's self type for an un-implemented trait method).

In practice, this won't affect existing tests because:
1. If a trait method has no body and is called via Param(N) receiver
   (Stage 32.3 path), the MIR lowerer generates the Call terminator
   but no real impl exists → codegen emits an extern declaration → no
   link error if never called (matches Stage 32.3 behavior).
2. typeck's arg-count check (line 527-537 in check.rs) only compares
   counts, not types — so Error in sig.inputs[0] is fine.
3. typeck's arg-type unification (line 539-555) might trigger a type
   mismatch error if it tries to unify self arg's type with Error — but
   for Param(N) receivers, the self arg is already Infer/Param, which
   unifies cleanly with Error (no false positive).

## 5. Test Plan (§9.4 + §7.3.1 ≥30 case audit)

### 5.1 Positive Tests (≥5)

| # | Source | Validates |
|---|--------|-----------|
| P1 | `trait T { fn f(&self, a: i32, b: i32) -> i32; } impl T for S { fn f(&self, a: i32, b: i32) -> i32 { 0 } } fn main() { let s = S; s.f(1, 2); }` | Concrete impl correct arg count |
| P2 | `trait T { fn f(&self, a: i32, b: i32) -> i32 { 0 } } impl T for S {} fn main() { let s = S; s.f(1, 2); }` | Default body correct arg count |
| P3 | `trait T { fn f(&self); } impl T for S { fn f(&self) {} } fn main() { let s = S; s.f(); }` | No-arg method |
| P4 | `trait T { fn f(&self, a: i32) -> i32; } struct S<X: T> { x: X } impl<X: T> S<X> { fn g(&self) -> i32 { self.x.f(1) } } fn main() { 0 }` | Param(N) receiver, correct arg count |
| P5 | `trait T { fn f(&self, a: i32, b: i32, c: i32) -> i32; } impl T for S { fn f(&self, a: i32, b: i32, c: i32) -> i32 { 0 } } fn main() { let s = S; s.f(1, 2, 3); }` | Multi-arg correct |

### 5.2 Negative Tests (≥28 covering 7 error categories)

| # | Category | Source |
|---|----------|--------|
| N1 | Typeck | `trait T { fn f(&self, a: i32, b: i32) -> i32; } impl T for S { fn f(&self, a: i32, b: i32) -> i32 { 0 } } fn main() { let s = S; s.f(1); }` |
| N2 | Typeck | `trait T { fn f(&self, a: i32, b: i32) -> i32; } impl T for S { fn f(&self, a: i32, b: i32) -> i32 { 0 } } fn main() { let s = S; s.f(1, 2, 3); }` |
| N3 | Typeck | `trait T { fn f(&self); } impl T for S { fn f(&self) {} } fn main() { let s = S; s.f(99); }` |
| N4 | Typeck | `trait T { fn f(&self, a: i32, b: i32) -> i32; } struct S<X: T> { x: X } impl<X: T> S<X> { fn g(&self) -> i32 { self.x.f(1) } } fn main() { 0 }` (case 1a — bug) |
| N5 | Typeck | `trait T { fn f(&self, a: i32, b: i32) -> i32 { 0 } } struct S<X: T> { x: X } impl<X: T> S<X> { fn g(&self) -> i32 { self.x.f(1) } } fn main() { 0 }` (case 1b — should still work) |
| N6 | Typeck | `trait T { fn f(&self, a: i32) -> i32; } struct S<X: T> { x: X } impl<X: T> S<X> { fn g(&self) -> i32 { self.x.f() } } fn main() { 0 }` (missing arg) |
| N7 | Typeck | `trait T { fn f(&self) -> i32; } struct S<X: T> { x: X } impl<X: T> S<X> { fn g(&self) -> i32 { self.x.f(99) } } fn main() { 0 }` (extra arg) |
| N8 | Typeck | `trait T { fn f(&self, a: i32, b: i32, c: i32) -> i32; } struct S<X: T> { x: X } impl<X: T> S<X> { fn g(&self) -> i32 { self.x.f(1, 2) } } fn main() { 0 }` (missing 1 of 3) |
| N9-N15 | Typeck | 7 more arg-count variants |
| N16-N18 | Lex | invalid tokens |
| N19-N21 | Parse | missing semis, braces, arrows |
| N22-N24 | Borrowck / Resolve / Trait | Other error categories |
| N25-N28 | Codegen / Nested / Context | Remaining categories |

Final count: 5 positive + 28 negative = 33 cases.

## 6. §14.5 Verification Plan

- D1 (fmt): clean
- D2 (clippy): 0 warnings
- D3 (build): success
- D4 (lib tests): 898/898
- D5 (integration tests): 4230+33 = 4263 (4 ignored)
- D6 (no P0/P1): TD resolved
- D7 (architecture health): 9.85/10 (stable)
- D8 (§1.6 终极检验): root-cause fix

## 7. Implementation Plan

1. Add `populate_trait_decl_fn_sigs` to `src/driver/driver_codegen_prep.rs`.
2. Call it in `compile_inner` after `populate_trait_default_fn_sigs`.
3. Create `tests/v0/stage35/plan/typeck_param_arg_count_tests.rs` with 5 positive + 28 negative tests.
4. Add module entry to `tests/all_tests.rs`.
5. Run §3.2 verification.
6. Update docs (worklog, tech-debt-register, RELEASE_NOTES, README, lang-design).
7. Package per §19.

## 8. References

- Rust Reference §Expressions — Method-call: https://doc.rust-lang.org/reference/expressions.html#method-call-expressions
- rustc `check_arg_count`: rustc_hir_typeck/src/check/call.rs
- TD-TYPECK-PARAM-ARG-COUNT definition: `docs/develop/v0/tech-debt-register.md:1062`
- Existing trait default fn_sig population: `src/driver/driver_codegen_prep.rs:322-494`
- Typeck Call terminator check: `src/typeck/check.rs:495-581`
