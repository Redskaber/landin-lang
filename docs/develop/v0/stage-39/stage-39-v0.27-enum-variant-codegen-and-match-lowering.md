# Stage 39 — v0.27 Enum Variant Codegen & Match Lowering Series

## Overview

Stage 39 (v0.27) resolves the enum variant codegen and match lowering bugs
that blocked prelude methods using `match *self { Some(_) => ...,
None => ... }` patterns. The series spans four stages (39, 39.1, 39.2, 39.3),
each addressing a distinct root cause.

## Stage 39.3 (v0.588.0) — Three Root-Cause Fixes (CURRENT)

### Goal

Unblock the prelude's `Option::is_some`, `Option::is_none`, and
`Option::unwrap_or` methods (and any prelude method using
`match *self { Some(_) => ..., None => ... }` patterns).

### Root Causes Fixed

1. **TD-LEXER-UNDERSCORE** (Lex layer):
   - **Symptom**: `Some(_)` parsed as `Some(<binding "_">)` instead of
     `Some(<wild>)`. MIR lowerer computed `has_inner_subpatterns = true`,
     preventing the variant from being added as a switch target.
   - **Root cause**: `lex_ident` returned `TokenKind::Ident("_")` for a
     lone underscore.
   - **Fix**: In `src/lexer/ident.rs`, check `text == "_"` after collecting
     the identifier and return `TokenKind::Underscore`.

2. **TD-PAT-IDENT-VARIANT** (Resolver layer):
   - **Symptom**: `match v { None => ... }` treated `None` as a catch-all
     binding.
   - **Root cause**: Parser unconditionally converted single-segment paths
     to `Pat::Ident`. Resolver's `collect_pat_bindings` didn't check
     `variant_index`.
   - **Fix**: In `src/resolve/path_resolve.rs::collect_pat_bindings`, when
     `HirPatKind::Ident(name, None)` is encountered, check `variant_index`.
     If found, convert the Ident pattern to a Path pattern with
     `res = Res::Def(enum_def_id, DefKind::Enum)`.

3. **TD-TEXT-IR-DEREF-ADT** (Codegen layer):
   - **Symptom**: TextEmitter IR for `match *self { ... }` (where
     `self: &Option<T>`) was rejected by `llvm-as` with type mismatch
     (`%v3` defined as `ptr` but expected `{ i32, i32 }`).
   - **Root cause**: `detect_place_type` for `Projection(base, Deref)`
     returned `OpaquePtr` (Stage 18.337 — to break recursive struct cycles).
     But `OpaquePtr.pointee() == OpaquePtr`, so the load `*self` used type
     `ptr` instead of the Adt's struct type.
   - **Fix**: In `src/codegen/mir_translation/places.rs::detect_place_type`,
     when the resolved EmitType is `OpaquePtr`, fall back to the MIR type
     via `resolve_base_ty_for_substs` and convert the underlying
     `Ref(_, _, inner)` to its proper EmitType.

4. **Additional fix — Binding sub-pattern classification**:
   - **Symptom**: `Some(v)` (with binding `v`) had `has_inner_subpatterns = true`
     because `v` was treated as differentiating. This prevented the variant
     from being added as a switch target, causing the otherwise block to be
     unreachable (segfault at runtime).
   - **Root cause**: Stage 14.89 (Bug 4 fix) classified any non-Wild sub-pattern
     as differentiating. But bindings (`HirPatKind::Ident`) always match —
     they're not differentiating.
   - **Fix**: In `pattern_lower.rs::has_inner_subpatterns`, treat
     `HirPatKind::Ident(..)` as non-differentiating (same as
     `HirPatKind::Wild`).

### Design Decisions (§12 最优 > 最小, 通解 > 特解)

- **Layered root-cause fixes**: Each fix targets the layer where the bug
  originates (lexer for `_`, resolver for `None`, codegen for `*self`,
  pattern_lower for `Some(v)` binding classification). No call-site patches.
- **Generic mechanisms**: Each fix applies to all enum types (Option, Result,
  user-defined), not just the prelude's Option.
- **Explicit error reporting**: All fixes preserve explicit error reporting
  paths (no silent fallbacks).

### Verification

- **Runtime**: `Some(42).is_some() == true`, `None.is_some() == false`,
  `Some(42).unwrap_or(99) == 42`, `None.unwrap_or(99) == 99`.
- **TextEmitter IR**: `llvm-as` accepts the IR (no type mismatches).
- **Tests**: 5415 total (898 lib + 4517 integration), 0 failures, 4 ignored.
- **fmt clean**, **0 clippy warnings**.

## Stage 39.2 (v0.587.0) — Scrutinee Type Resolution

### Goal

Fix the `is_enum` check that failed when `scrut_ty` was `Infer` (typeck hadn't
resolved the enum type yet at MIR lower time).

### Fix

In `src/mir/lower/pattern_lower.rs`, when `is_enum` is true but `scrut_ty` is
Infer or Error, resolve the enum DefId from the first arm pattern that has
`Res::Def(_, DefKind::Enum)`. Construct the Adt type and update the
scrutinee local's type so the discriminant extraction works.

### Known Limitation (resolved in 39.3)

The `Some` variant match arm was not reached because switch targets list was
built before the type fix was applied. Stage 39.3 resolves this by fixing
the underlying lexer/resolver issues.

## Stage 39.1 (v0.586.0) — Enum Match Pattern Lowering for Single-Segment Paths

### Goal

Support single-segment paths like `None` in match patterns and unify
`ConstVal::Uint → ConstVal::Int` for enum discriminants.

### Fix

In `src/mir/lower/pattern_lower.rs`, the `enum_variant_idx` resolution was
updated to support single-segment paths via the same
`!path.segments.is_empty()` check used in Stage 39 for variant construction.
`ConstVal::Uint` was changed to `ConstVal::Int` for all enum discriminant
values to ensure consistency between variant construction and match pattern
switch targets.

## Stage 39 (v0.585.0) — Enum Variant Codegen for Single-Segment Paths

### Goal

Fix the enum variant codegen bug for single-segment paths like `None`/`Some`
from prelude body (discovered in Stage 38.2). Re-enable Vec::pop.

### Fix

In `src/mir/lower/expr_variants.rs`, `lower_path_expr` checked
`path.segments.len() >= 2` but `None` from prelude body is single-segment.
Fix: `!path.segments.is_empty()`.

### Known Limitation (resolved in 39.3)

`Option::is_some` from prelude body returned wrong value at runtime —
match lowering issue in generic context.

## Test Coverage Summary

| Stage | Tests Added | Cumulative Total |
|-------|-------------|------------------|
| 39    | 0 (re-enabled Vec::pop, no new tests) | 5392 |
| 39.1  | 0 (bug fix only, no new tests) | 5392 |
| 39.2  | 0 (bug fix only, no new tests) | 5392 |
| 39.3  | 23 (8 positive + 24 negative - 9 updated lexer tests) | 5415 |

## Next Steps

With prelude `Option::is_some`/`is_none`/`unwrap_or` now working, the
following prelude methods are unblocked:

- `Option::map` — uses `match self { Some(v) => Some(f(v)), None => None }`
- `Option::and_then` — uses `match self { Some(v) => f(v), None => None }`
- `Result::map`, `Result::and_then` — similar patterns

The next MUV (Stage 40) will add these prelude methods and verify them
via runtime tests.
