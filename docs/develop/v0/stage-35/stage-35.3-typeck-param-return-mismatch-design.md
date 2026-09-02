# Stage 35.3 (v0.23) — TD-TYPECK-PARAM-RETURN-MISMATCH Design

> **Author**: redskaber (PM-A + ARCH-A + DEV-A)
> **Date**: 2026-09-01
> **Version**: v0.576.0 (target)
> **Process**: stage-committee-process.md v7.5 §13.1 + §14.8
> **Complexity**: L2 (~30 LOC code + ~330 LOC tests + ~200 LOC docs)

## 1. Executive Summary

TD-TYPECK-PARAM-RETURN-MISMATCH (P3, documented since Stage 32.3) is resolved.
The bug: typeck silently accepted type mismatches when assigning a concrete
rvalue to a `Param(N)`-typed place. This affects:

1. Generic fn/method **return type** mismatch: `fn f<T>(x: T) -> T { true }` —
   returns `bool` but sig says `T`.
2. Generic let-binding mismatch: `fn f<T>() { let y: T = true; }` — assigns
   `bool` to `T`-typed local.
3. Generic field assignment mismatch: `impl<T> S<T> { fn f(&mut self) { self.x = true; } }`
   — assigns `bool` to `T`-typed field.

**Root cause**: `src/typeck/check.rs:80` has `let place_has_param =
type_contains_param_recursive(&resolved_place);` and the check at line 93
**skips** the mismatch check when `place_has_param` is true (per Stage 18.351
"defer to writeback" rationale).

**Why the skip is wrong (per §2.2 根因思维)**:
- Writeback only substitutes `Param(N)` with concrete types from call-site
  substs — it does NOT compare the rvalue type with the resolved return type.
- So a concrete rvalue (e.g., `true` of type `bool`) assigned to a
  `Param(0)`-typed place (returning `T`) is NEVER validated — writeback
  keeps the bool→Param assignment as-is, codegen generates wrong code.
- The Stage 18.351 comment said "writeback will resolve the Param via
  Rule 3 Field projection's substitute() call" — but this only applies
  to field accesses via `Place::Projection(Field)`, NOT to direct
  Assign to a Param-typed local (return value or let-binding).

## 2. Bug Confirmation (runtime evidence)

Verified via `examples/test_return_mismatch.rs`:

| Case | Source | Expected | Actual (v0.575.0) | Status |
|------|--------|----------|-------------------|--------|
| 1 | `impl<T> S<T> { fn get_wrong(&self) -> T { true } }` | ERROR | 0 errors | ❌ Silent |
| 2 | `impl S { fn get_wrong(&self) -> i32 { true } }` (non-generic) | ERROR | 1 error | ✅ |
| 3 | `impl<T> S<T> { fn get(&self) -> T { self.x } }` | OK | 0 errors | ✅ |
| A | `impl<T> S<T> { fn set_wrong(&mut self) { self.x = true; } }` | ERROR | 0 errors | ❌ Silent |
| B | `fn f<T>(x: T) -> T { let y: T = true; y }` | ERROR | 0 errors | ❌ Silent |
| C | `fn f<T>(x: T) -> T { true }` | ERROR | 0 errors | ❌ Silent |
| D | `fn f<T>(x: T) -> T { let y: T = x; y }` (legitimate) | OK | 0 errors | ✅ |
| E | `fn id<T>(x: T) -> T { x }` (legitimate) | OK | 0 errors | ✅ |
| F | `impl<T> S<T> { fn get(&self) -> T { self.x } }` (legitimate) | OK | 0 errors | ✅ |

Cases 1, A, B, C are silent bugs (concrete rvalue assigned to Param-typed place).
Cases D, E, F are legitimate (rvalue is Infer or Param, unifies cleanly).

## 3. Rust Reference Design Alignment

Per [Rust Reference §Functions](https://doc.rust-lang.org/reference/items/functions.html):
> The body of a function must have a type that unifies with the function's
> return type. If not, the compiler reports E0308 ("mismatched types").

Per rustc: `check_return_expr` in `rustc_hir_typeck/src/check/fn.rs` unifies
the body's expr type with the return type — for generic functions, this
unification happens with the generic param's inference variable (which
constrains the param at the call site).

**Rust philosophy applied**:
- §1.0 原則 4 (报错 > 静默): report concrete-vs-Param mismatch.
- §1.0 原則 6 (通解 > 特解): one check at `post_check_statement` covers all
  3 bug cases (return, let-binding, field assignment).
- §1.0 原則 9 (正确 > 妥协): don't defer to writeback — writeback doesn't
  validate types, only substitutes.
- §1.0 原則 11 (确定性边界): the boundary is "concrete rvalue vs Param place"
  — explicit and unambiguous.
- §12 (最优 > 最小): root-cause fix = remove the over-broad skip in
  `post_check_statement`.

## 4. Design

### 4.1 Fix Location

`src/typeck/check.rs:80-93` — the `place_has_param` skip is too broad.
Narrow it: skip ONLY when rvalue is also Param/Infer (which legitimately
unifies). When rvalue is CONCRETE, report the mismatch.

### 4.2 New Check Logic

Replace the existing block at lines 80-121 with:

```rust
let place_has_param = type_contains_param_recursive(&resolved_place);

let place_is_concrete = /* existing */;
let rvalue_is_concrete = /* existing */;

// Stage 35.3 (v0.23 — TD-TYPECK-PARAM-RETURN-MISMATCH): When place is
// Param(N) (e.g., return value of generic fn, let-binding of T-typed
// local, field assignment of T-typed field) AND rvalue is concrete,
// report the mismatch. Previously this was silently skipped (Stage
// 18.351 "defer to writeback"), but writeback only substitutes Param
// via field projection — it does NOT validate concrete-vs-Param
// assignments to direct locals.
//
// Per §1.0 原則 4 (报错 > 静默): fix the silent skip.
// Per §1.0 原則 6 (通解 > 特解): one check covers return/let/field
// assignments uniformly.
// Per §1.0 原則 9 (正确 > 妥协): don't defer to writeback — it doesn't
// validate types.
let should_check_concrete_vs_param =
    place_has_param && rvalue_is_concrete && !can_coerce(&resolved_place, &resolved_rvalue);

let should_check_concrete_vs_concrete =
    !place_has_param && place_is_concrete && rvalue_is_concrete
        && !can_coerce(&resolved_place, &resolved_rvalue)
        && !types_match_loose(&resolved_place, &resolved_rvalue);

if should_check_concrete_vs_param || should_check_concrete_vs_concrete {
    // Dedup + report (existing logic).
    let span = stmt.span;
    let already_reported = self.errors.iter().any(|e| {
        e.span == span
            && e.expected.as_ref() == Some(&resolved_place)
            && e.found.as_ref() == Some(&resolved_rvalue)
    });
    if !already_reported {
        self.errors.push(crate::typeck::TypeError::mismatch(
            resolved_place.clone(),
            resolved_rvalue.clone(),
            span,
        ));
    }
}
```

### 4.3 Why Not Run typeck AFTER Writeback?

Per §1.0 原則 11 (确定性边界): typeck runs before writeback by design —
typeck propagates type constraints to infer table, writeback consumes
them. Running typeck after writeback would break the existing pipeline
(per §16 管道流 — type errors belong in typeck, not writeback).

### 4.4 Why Not Use can_coerce?

The existing check uses `can_coerce` — we keep using it for the
concrete-vs-Param case too. For `bool` vs `Param(0)`, `can_coerce` returns
false → mismatch reported. For `Infer` vs `Param(0)`, the rvalue isn't
concrete → not in our new branch. So no false positive.

### 4.5 What About `types_match_loose`?

The existing `types_match_loose` returns true for some "loose" matches
(e.g., int variants). For concrete vs Param, this returns false (Param
is not a "loose match" for bool). So no false positive.

## 5. Test Plan (§9.4 + §7.3.1 ≥30 case audit)

### 5.1 Positive Tests (≥5)

| # | Source | Validates |
|---|--------|-----------|
| P1 | `fn id<T>(x: T) -> T { x }` | Generic id (Infer rvalue) |
| P2 | `impl<T> S<T> { fn get(&self) -> T { self.x } }` | Generic field return (Param rvalue) |
| P3 | `fn f<T>(x: T) -> T { let y: T = x; y }` | Generic let binding (Infer rvalue) |
| P4 | `impl<T> S<T> { fn set(&mut self, v: T) { self.x = v; } }` | Generic field assign (Infer rvalue) |
| P5 | `fn id<T>(x: T) -> T { let y: T = x; y } fn main() { let _ = id::<i32>(5); }` | E2E |

### 5.2 Negative Tests (≥28 covering 7 error categories)

| # | Category | Source |
|---|----------|--------|
| N1 | Typeck | `fn f<T>(x: T) -> T { true }` (Case C — bug) |
| N2 | Typeck | `impl<T> S<T> { fn g(&self) -> T { true } }` (Case 1 — bug) |
| N3 | Typeck | `fn f<T>(x: T) -> T { let y: T = true; y }` (Case B — bug) |
| N4 | Typeck | `impl<T> S<T> { fn set_wrong(&mut self) { self.x = true; } }` (Case A — bug) |
| N5 | Typeck | Non-generic equivalent returns wrong type |
| N6-N15 | Typeck | 10 more type mismatch variants |
| N16-N18 | Lex | invalid tokens |
| N19-N21 | Parse | missing semis, braces, arrows |
| N22 | Borrowck | double mut borrow |
| N23-N24 | Resolve | Self outside impl, undefined type |
| N25-N26 | Trait | trait impl wrong sig, undefined trait |
| N27 | Codegen | extern call path |
| N28 | Nested | nested Param mismatch |

Total: 5 positive + 28 negative = 33 cases.

## 6. §3.2 Verification Plan

- cargo clean ✓
- cargo build --release ✓
- cargo check (0 errors, 0 warnings) ✓
- cargo fmt --check (0 diff) ✓
- cargo clippy -- -D warnings (0 warnings) ✓
- cargo test --release (5161+33 = 5194 tests, 0 failed) ✓

## 7. Implementation Plan

1. Modify `src/typeck/check.rs:80-121` — narrow the `place_has_param` skip.
2. Create `tests/v0/stage35/plan/typeck_param_return_mismatch_tests.rs` with 5+28 tests.
3. Add module entry to `tests/all_tests.rs`.
4. Run §3.2 verification.
5. Update docs (worklog, tech-debt-register, RELEASE_NOTES, README, lang-design).
6. Package per §19.

## 8. References

- Rust Reference §Functions: https://doc.rust-lang.org/reference/items/functions.html
- rustc E0308: https://doc.rust-lang.org/error_codes/E0308.html
- Existing Stage 18.351 skip rationale: `src/typeck/check.rs:62-79`
- TD-TYPECK-PARAM-RETURN-MISMATCH definition: `docs/develop/v0/tech-debt-register.md:1061`
- Writeback Phase 3 (Field projection substitute): `src/mir/lower/writeback.rs`
