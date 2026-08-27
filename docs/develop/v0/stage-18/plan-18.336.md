# Stage 18.336 — ZST Nested Aggregate Void Leak + Typeck Return/Trait Gaps

> **Author**: Super Z (main) — PM-A + ARCH-A + DEV-A + REV-A + QA-A
> **Date**: 2026-08-27
> **Complexity**: L3 (cross-module: codegen/mir_translation/{types,layouts}.rs + mir/lower/body_lower.rs + typeck/check.rs + driver/driver_validations.rs + tests)
> **Status**: planned → in-progress

## 1. 5W2H Analysis

### What
The §20 Round 5 audit (sub-agent, empirically verified via `landin_compiler::compile` +
`llvm-as` validation) found:

- **4 P1 NEW bugs (ZST Void leak in nested aggregates)** — same class as Stage 18.335
  ZST param elision, but at struct/tuple/enum/array element positions:
  - A1: `struct S { u: () }` → `alloca { void }` → `llvm-as` rejects.
  - A2: `let _t: (i32, ()) = (42, ())` → `alloca { i32, void }` → rejects.
  - A3: `enum E { V(()), W(i32) }` → `alloca { i32, void, i32 }` → rejects.
  - A4: `let _a: [(); 3] = [(), (), ()]` → `alloca [3 x void]` → rejects.

- **5 P2 NEW typeck gaps** (silent acceptance of type-incorrect code):
  - B2: `fn foo() -> S { 42 }` (struct return + Infer rvalue) → no error.
  - B3: `fn foo() -> () { true }` (ZST return + bool rvalue) → no error.
  - B4: `fn foo() { 42i64 }` (implicit `()` return + i64 rvalue) → no error.
  - C2: `trait T { fn f(&self); } impl T for X { fn f(self) {} }` (self_kind mismatch) → no error.
  - C3: `trait T { fn f() -> i32; } impl T for X { fn f() -> i64 { 0 } }` (int width) → no error.

- **2 P2 known gaps** (Stage 18.335 tests skip with warning):
  - B1: `fn foo() -> () { 42i64 }` (ZST return + i64 rvalue) → no error.
  - C1: `impl Drop for Foo { fn drop(self) {} }` (Drop self_kind mismatch) → no error.

### Why (root cause per §2.2 + §20)
- **A1-A4**: `mir_type_to_emit_type_with_layouts_and_mono` (and `_with_layouts`) maps
  `TyKind::Tuple(vec![])` → `EmitType::Void`. This is correct for first-class positions
  (return type, function param) — Stage 18.335 filters them out. But it's **wrong** for
  **nested** aggregate positions (struct field, tuple element, enum payload, array element).
  LLVM IR rejects `{ void }`, `[3 x void]`, etc. with "void type only allowed for function
  results".
- **B1/B3/B4**: `body_lower.rs:443-445` has `let skip_assign = cx.is_terminated() || return_is_unit;`
  — when the return type is `()`, the `Assign(return_local, Move(value_local))` is never
  emitted in MIR, so typeck never sees the type mismatch.
- **B2**: `typeck/check.rs:236` has `let _ = self.unify.unify(...)` — the unify error is
  silently discarded when the rvalue is `Infer` (which `can_coerce` returns true for).
- **C1/C2**: `driver_validations.rs:110-125` filters out `self_kind` from param comparison
  — so self receiver mismatches (`&mut self` vs `self`) are never compared.
- **C3**: `driver_validations.rs:226-233` `mir_ty_kinds_compatible` treats `i32` vs `i64`
  as "compatible" because both are `TyKind::Int(_)` — but `i32` and `i64` are NOT
  type-compatible in Rust (trait impls must match exactly).

### Who (roles per §1.4)
- ARCH-A: design the "filter Void from nested aggregates" pattern + 5 typeck fixes
- DEV-A: implement across 5 sites (mir_translation + body_lower + typeck/check + driver_validations)
- REV-A: review for soundness (no UB, no silent degradation)
- QA-A: regression + convert 2 skip-with-warning tests to hard assertions

### When
- Stop when §3.2 all-green AND `llvm-as` accepts IR for all 4 ZST aggregate repros
  AND the 7 typeck gap repros all report errors.

### Where
- `src/codegen/mir_translation/types.rs` — filter Void from Struct/Tuple/Array fields
- `src/codegen/mir_translation/layouts.rs` — filter Void from AdtLayout field_tys
- `src/mir/lower/body_lower.rs` — keep skip_assign for codegen but always emit mismatch check
- `src/typeck/check.rs` — remove `let _ = unify(...)` discard
- `src/driver/driver_validations.rs` — add self_kind comparison + tighten Int/Uint/Float match
- `tests/v0/stage18/plan/stage18_336_*.rs` — regression tests

### How (implementation strategy)

#### 4.1 A1-A4 fix: filter Void from nested aggregates

In `mir_translation/types.rs` and `layouts.rs`, filter `EmitType::Void` from
`Struct(vec)` field lists. If all fields are Void (fully ZST aggregate), keep as
`EmitType::Struct(vec![])` (LLVM `{}`, which IS valid as a struct type — the
issue is only with `Void` fields inside a struct).

```rust
TyKind::Tuple(tys) => {
    if tys.is_empty() {
        EmitType::Void
    } else {
        let fields: Vec<EmitType> = tys.iter()
            .map(|t| mir_type_to_emit_type_with_layouts_and_mono(t, layouts, mono_layouts))
            .filter(|ty| *ty != EmitType::Void)  // ZST tuple elements elided
            .collect();
        if fields.is_empty() {
            EmitType::Struct(vec![])  // all fields were ZST → {} (valid empty struct)
        } else {
            EmitType::Struct(fields)
        }
    }
}
```

Same for `AdtLayout::Struct { field_tys }`, `AdtLayout::Enum { variant_payloads }`,
and `Array(elem, n)` (use `Struct(vec![])` as the element type for ZST arrays →
`[3 x {}]` is valid LLVM).

Per §1.0 原則 6 (通解 > 特解): one fix covers all 4 cases (A1-A4 same class).
Per §20 (iterative audit): same root cause as Stage 18.335 ZST param elision.

#### 4.2 B1/B3/B4 fix: always emit return type mismatch check

In `body_lower.rs:443-445`, keep `skip_assign` for codegen (ZST return doesn't need
the assign), but always emit the typeck mismatch check separately. This decouples
typeck from codegen-skip.

Per §1.0 原則 4 (报错 > 静默): the mismatch must be reported, not silently skipped.

#### 4.3 B2 fix: remove `let _ = unify(...)` discard

In `typeck/check.rs:236`, replace `let _ = self.unify.unify(...)` with the error-push
pattern (same as the FnDef↔FnPtr branch above it).

Per §1.0 原則 5 (去除兼容思维): the suppression was a workaround; remove it.
Per §1.0 原則 4 (报错 > 静默): unify errors are real type mismatches that must be reported.

#### 4.4 C1/C2 fix: add self_kind comparison

In `driver_validations.rs:108` (after the non-self param filter), add a separate
self_kind comparison:

```rust
let trait_self = trait_fn.sig.inputs.iter().find_map(|p| p.self_kind.as_ref());
let impl_self = impl_fn.sig.inputs.iter().find_map(|p| p.self_kind.as_ref());
if trait_self != impl_self {
    errors.push(TypeError::new(
        format!("method `{}` self receiver mismatch: expected `{:?}`, found `{:?}`",
            method_name, trait_self, impl_self),
        impl_fn.span,
    ));
}
```

Per §1.0 原則 4 (报错 > 静默): self receiver kind must match between trait declaration and impl.

#### 4.5 C3 fix: tighten mir_ty_kinds_compatible

In `driver_validations.rs:226-233`, require exact Int/Uint/Float width match:

```rust
(TyKind::Int(a_i), TyKind::Int(b_i)) => a_i == b_i,  // same width only
(TyKind::Uint(a_u), TyKind::Uint(b_u)) => a_u == b_u,
(TyKind::Float(a_f), TyKind::Float(b_f)) => a_f == b_f,
(TyKind::Int(_), TyKind::Uint(_)) | (TyKind::Uint(_), TyKind::Int(_)) => false,
```

Per §1.0 原則 9 (正确 > 妥协): trait impls must match the declared signature exactly.
Per §2.0 原则 9: soundness — no implicit coercion in trait impl signatures.

### How Much (acceptance per §3.2)
- `cargo fmt --check` ✅ 0 diff
- `cargo check --features llvm-backend` ✅ 0 errors, 0 warnings
- `cargo clippy --all-targets --features llvm-backend -- -D warnings` ✅ 0 warnings
- `cargo test --release --features llvm-backend --test-threads=1` ✅ 0 failures
- Multi-threaded stress: ≥4/5 stable (2 threads)
- **NEW**: `llvm-as` accepts TextEmitter IR for all 4 ZST aggregate repros (A1-A4).
- **NEW**: All 7 typeck gap repros (B1-B4, C1-C3) report errors.

## 2. Decision Points (per §2.2 + §12)

### 2.1 Why filter Void from nested aggregates (A) vs. map ZST to Struct(vec![]) everywhere (B)?
- **(B) Map ZST to Struct(vec![]) everywhere**: would change the semantic meaning of
  `EmitType::Void` (used for true void returns). Also, the existing `i8` fallback for
  ZST allocas (Stage 16.22) handles the alloca case — we don't need to change that.
- **(A) Filter Void from nested aggregates**: only changes the **nested** position
  (struct field, tuple element, etc.). Top-level ZST params/returns still use Void
  (correctly filtered by Stage 18.335). This is the minimal, root-cause fix.
- **§1.0 原則 6 (通解 > 特解)**: filter Void from nested aggregates is the GENERIC
  pattern; mapping to Struct(vec![]) is a special-case that conflates ZST with non-ZST.

### 2.2 Why keep `skip_assign` for codegen but always run typeck check (B1/B3/B4)?
- The `skip_assign` is needed for codegen: ZST returns don't need the assign
  (no value to store). Removing it would produce `store void %v, ptr %loc_0` → invalid IR.
- But the typeck check should ALWAYS run, regardless of codegen needs.
- **§1.0 原則 4 (报错 > 静默)**: the mismatch must be reported, not silently skipped.
- **§2.2 (根因思维)**: decouple typeck from codegen-skip — the root cause was conflating them.

### 2.3 Why remove `let _ = unify(...)` discard (B2)?
- The discard was added to suppress "spurious unify errors" during coercion. But
  coercion should succeed unify (no error to suppress). Only `FnDef↔FnPtr` was the
  special case (already handled separately).
- The discard masks real type mismatches like `fn foo() -> S { 42 }`.
- **§1.0 原則 5 (去除兼容思维)**: the suppression was a workaround; remove it.
- **§1.0 原則 4 (报错 > 静默)**: unify errors are real type mismatches that must be reported.

## 3. Capability / Design / Responsibility Boundaries

### 3.1 Capability boundary
- `EmitType::Void` is unchanged — still used for true void returns and top-level ZST.
- The codegen layer filters Void from **nested** aggregate positions (struct/tuple/enum/array).
- Typeck always runs the return-type mismatch check, regardless of codegen-skip.

### 3.2 Design boundary
- ZST fields are elided from LLVM struct types (mirror rustc's ZST field elision).
- ZST array elements use `Struct(vec![])` (LLVM `{}`) → `[3 x {}]` is valid (zero-size array).
- Trait impl signatures must match the declared signature exactly (no implicit coercion).

### 3.3 Responsibility boundary
- `mir_translation/types.rs` + `layouts.rs`: filter Void from nested aggregates.
- `mir/lower/body_lower.rs`: keep skip_assign for codegen, always run typeck check.
- `typeck/check.rs`: remove `let _ = unify(...)` discard.
- `driver/driver_validations.rs`: add self_kind comparison + tighten Int/Uint/Float match.
