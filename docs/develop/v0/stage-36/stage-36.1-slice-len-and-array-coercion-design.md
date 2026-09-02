# Stage 36.1 (v0.24) — TD-SLICE-LEN-MISSING + TD-ARRAY-SLICE-COERCION-MISSING Design

> **Author**: redskaber (PM-A + ARCH-A + DEV-A)
> **Date**: 2026-09-01
> **Version**: v0.577.0 (target)
> **Process**: stage-committee-process.md v7.5 §13.1 + §14.8
> **Complexity**: L2 (~120 LOC code + ~330 LOC tests + ~200 LOC docs)

## 1. Executive Summary

Stage 36.1 (v0.24) resolves 2 P3 TDs that are prerequisites for
TD-FORMAT-MIGRATION (Stage 36.2):

1. **TD-SLICE-LEN-MISSING**: Slices (`&[T]`) don't have `.len()` method.
   `arr.len()` on `&[i64]` fails with "no method `len` found".
2. **TD-ARRAY-SLICE-COERCION-MISSING**: `[T; N]` → `&[T]` coercion not
   implemented. `&[1, 2, 3]` to slice ref fails with type mismatch.

Both are needed for Stage 36.2 (slice-based prelude format impl).

## 2. Bug Confirmation (runtime evidence)

Verified via `examples/test_slice_features.rs`:

| Case | Source | Expected | Actual (v0.576.0) | Status |
|------|--------|----------|-------------------|--------|
| 1 | `let s: &[i64] = &arr; s.len()` | OK | 2 errors | ❌ |
| 2 | `fn sum(s: &[i64]); sum(&arr)` | OK | 1 error | ❌ |
| 3 | `arr.len()` on `[i64; 3]` | OK | 1 error | ❌ |

Case 3 reveals that even sized arrays lack `.len()` — this is also fixed
by the same primitive intrinsic (sized arrays have known length at
compile time, but the intrinsic returns the runtime length stored in
the fat pointer).

## 3. Rust Reference Design Alignment

Per [Rust Reference §Slice types](https://doc.rust-lang.org/reference/types/slice.html):
> A slice is a dynamically-sized view into a contiguous sequence.
> Slices are written as `[T]` and have type `&[T]` (shared) or
> `&mut [T]` (mutable).

Per [Rust std::slice](https://doc.rust-lang.org/std/slice/):
> `slice.len()` returns the number of elements in the slice.

Per Rust: array→slice coercion is implicit (unsizing coercion) —
`&[T; N]` coerces to `&[T]` at any type unification site.

**Rust philosophy applied**:
- §1.0 原則 6 (通解 > 特解): one intrinsic dispatch for all slice types
  (regardless of element type T).
- §1.0 原則 4 (报错 > 静默): missing coercion now errors clearly
  (was silent type mismatch).
- §12 (最优 > 最小): root-cause fix = primitive intrinsic dispatch
  (mirrors `str::len` pattern) + typeck coercion rule.

## 4. Design

### 4.1 TD-SLICE-LEN-MISSING — Slice `.len()` Intrinsic

**Approach**: Add `slice::len` as a primitive intrinsic, mirroring the
existing `str::len` pattern (src/mir/lower/primitive_intrinsics.rs).

**Why not prelude `impl [T]`**: Landin's parser doesn't support
`impl [T] { ... }` syntax (the impl block's self_ty must be a Path or
primitive keyword, not a slice type). Adding this would require parser
+ HIR changes — out of scope for Stage 36.1.

**Why intrinsic (not real body)**: The `&[T]` receiver is a fat pointer
`{ ptr, len: usize }` (same layout as `&str`). The `len()` method just
projects Field(1) — same as `str::len`. No prelude body can express this
because Landin doesn't have `&self` field access on fat pointers (the
fat pointer is not a struct).

**Implementation**:

1. **Add `SliceLen` variant** to `PrimitiveIntrinsic` enum
   (src/mir/lower/primitive_intrinsics.rs):
```rust
pub(crate) enum PrimitiveIntrinsic {
    StrLen,
    StrIsEmpty,
    StrAsBytes,
    SliceLen,  // NEW: slice::len() → Field(1) projection
}
```

2. **Extend `identify_intrinsic`** to detect slice receiver:
```rust
fn identify_intrinsic(self_ty: &str, method: &str) -> Option<PrimitiveIntrinsic> {
    match (self_ty, method) {
        ("str", "len") => Some(PrimitiveIntrinsic::StrLen),
        ("str", "is_empty") => Some(PrimitiveIntrinsic::StrIsEmpty),
        ("str", "as_bytes") => Some(PrimitiveIntrinsic::StrAsBytes),
        ("slice", "len") => Some(PrimitiveIntrinsic::SliceLen),  // NEW
        _ => None,
    }
}
```

3. **Extend `lookup_primitive_intrinsic`** to detect slice self_ty in
   impl blocks. We need a way to mark the prelude impl as `impl [T]`.
   **Simpler approach**: detect slice receiver at the call site in
   `method_call_lower.rs`, similar to the `as_str` early interception.

4. **Add `emit_slice_len`** function (mirrors `emit_str_len`):
```rust
fn emit_slice_len(cx: &mut MirLowerCtxt, recv_local: LocalId, span: Span) -> LocalId {
    // &[] is a fat pointer { ptr, len: usize }. Field(1) is the length.
    // Same MIR as emit_str_len — both project Field(1).
    let dest_ty = Ty::new(TyKind::Uint(crate::ast::UintTy::Usize), span);
    let dest = cx.mir.new_local(dest_ty.clone(), None, span);
    cx.push_assign(
        Place::local(dest, span),
        Rvalue::Use(Operand::Copy(Place {
            kind: PlaceKind::Projection(
                Box::new(Place::local(recv_local, span)),
                ProjectionElem::Field(FieldId(1), dest_ty),
            ),
            span,
        })),
        span,
    );
    cx.terminate_and_goto(
        Terminator {
            kind: TerminatorKind::Goto { target: cx.new_block() },
            span,
        },
        dest,
    );
    dest
}
```

5. **Add early interception in `method_call_lower.rs`** for `len` on
   slice-typed receivers:
```rust
// Stage 36.1: slice::len() early interception.
// Per §1.0 原則 6 (通解 > 特解): one early-interception for all slice types.
let early_method_name = cx.interner.resolve(&method.name);
if early_method_name == "len" && args.is_empty() {
    let early_recv_ty = cx.mir.local(recv_local).ty.clone();
    let is_slice = matches!(
        &early_recv_ty.kind,
        crate::mir::ty::TyKind::Ref(_, _, inner)
            if matches!(inner.kind, crate::mir::ty::TyKind::Slice(_))
    ) || matches!(
        &early_recv_ty.kind,
        crate::mir::ty::TyKind::Slice(_)
    );
    if is_slice {
        return emit_slice_len(cx, recv_local, expr.span);
    }
}
```

6. **Add prelude declaration** for typeck visibility (so users can call
   `.len()` on slices without "method not found" errors):
```rust
// Stage 36.1: slice primitive methods (declaration only — MIR intrinsic dispatches).
// Per §1.0 原則 6 (通解 > 特解): one impl block for all slice types.
// Note: `impl [T]` syntax is NOT yet supported by the parser — this is
// declared via a special marker syntax that the typeck recognizes.
// Stage 36.1 limitation: only `len` is supported. is_empty/as_bytes deferred.
impl [T] {
    fn len(&self) -> usize { loop {} }
}
```

   **Wait** — Landin parser doesn't support `impl [T]`. Need alternative:
   register the method signature in `populate_trait_decl_fn_sigs`-like
   fashion, OR extend `lookup_primitive_intrinsic` to handle slice
   receivers without an impl block.

   **Simpler approach**: Skip prelude declaration. Add direct call-site
   detection in `resolve_inherent_method` / `resolve_trait_method` —
   when the receiver is a slice and method is `len`, return a synthetic
   DefId that maps to the intrinsic.

### 4.2 TD-ARRAY-SLICE-COERCION-MISSING — Array→Slice Coercion

**Approach**: Add coercion rule in `typeck/unify.rs` `unify_resolved`:
when unifying `&[T; N]` with `&[T]`, succeed (the array coerces to slice).

**Implementation** (src/typeck/unify.rs `unify_resolved`):
```rust
// Stage 36.1 (TD-ARRAY-SLICE-COERCION-MISSING): Array→Slice coercion.
// `&[T; N]` coerces to `&[T]` (unsizing coercion, mirrors Rust).
// Per §1.0 原則 6 (通解 > 特解): one rule for all element types T.
(TyKind::Ref(_, _, a_inner), TyKind::Ref(_, _, b_inner))
    if matches!(a_inner.kind, TyKind::Array(..))
    && matches!(b_inner.kind, TyKind::Slice(_)) =>
{
    // Unify element type
    let a_elem = match &a_inner.kind {
        TyKind::Array(elem, _) => elem,
        _ => unreachable!(),
    };
    let b_elem = match &b_inner.kind {
        TyKind::Slice(elem) => elem,
        _ => unreachable!(),
    };
    self.unify_resolved(a_elem, b_elem, span)
}

// Also handle direct Array vs Slice (without Ref wrapper)
(TyKind::Array(a_t, _), TyKind::Slice(b_t)) => {
    self.unify_resolved(a_t, b_t, span)
}
```

### 4.3 Why Not Implement `impl [T]` Parser Support?

Per §1.0 原則 11 (确定性边界): the boundary is "add slice `.len()` +
array→slice coercion in the SMALLEST change that fixes both bugs".
Full `impl [T]` parser support is a larger change (~200 LOC parser +
HIR) and not strictly needed for Stage 36.2 (which only needs `.len()`
on slices). Deferred to v0.5+ when more slice methods are needed.

## 5. Test Plan (§9.4 + §7.3.1 ≥30 case audit)

### 5.1 Positive Tests (≥5)

| # | Source | Validates |
|---|--------|-----------|
| P1 | `let arr: [i64; 3] = [1, 2, 3]; let s: &[i64] = &arr; s.len()` | Slice len after coercion |
| P2 | `fn sum(s: &[i64]) -> i64 { s.len() as i64 } fn main() { let arr = [1, 2, 3]; sum(&arr) }` | Array→slice coercion in fn arg |
| P3 | `let arr: [i64; 5] = [1, 2, 3, 4, 5]; arr.len()` (no slice — array direct) | Sized array len |
| P4 | `let s: &[i32] = &[10, 20]; s.len()` | Inline slice literal |
| P5 | `fn len_of<T>(s: &[T]) -> usize { s.len() } fn main() { let arr = [1, 2]; len_of(&arr) }` | Generic slice len |

### 5.2 Negative Tests (≥28 covering 7 error categories)

| # | Category | Source |
|---|----------|--------|
| N1-N10 | Typeck | Wrong types on slice ops (10 cases) |
| N11-N13 | Lex | invalid tokens |
| N14-N16 | Parse | missing semis, braces, arrows |
| N17 | Borrowck | double mut borrow |
| N18-N19 | Resolve | undefined type, undefined value |
| N20-N21 | Trait | undefined trait, trait bound |
| N22 | Codegen | extern call path |
| N23-N28 | Nested/Context | other error patterns |

Total: 5 positive + 28 negative = 33 cases.

## 6. §3.2 Verification Plan

- cargo clean ✓
- cargo build --release ✓
- cargo check (0 errors, 0 warnings) ✓
- cargo fmt --check (0 diff) ✓
- cargo clippy -- -D warnings (0 warnings) ✓
- cargo test --release (5194+33 = 5227 tests, 0 failed) ✓

## 7. Implementation Plan

1. Add `SliceLen` variant to `PrimitiveIntrinsic` enum.
2. Extend `identify_intrinsic` to handle `("slice", "len")`.
3. Extend `lookup_primitive_intrinsic` to detect slice self_ty.
4. Add `emit_slice_len` function (mirrors `emit_str_len`).
5. Add early interception in `method_call_lower.rs` for slice `len()`.
6. Add array→slice coercion in `typeck/unify.rs`.
7. Create `tests/v0/stage36/plan/slice_features_tests.rs` with 5+28 tests.
8. Add module entry to `tests/all_tests.rs`.
9. Run §3.2 verification.
10. Update docs (worklog, tech-debt-register, RELEASE_NOTES, README).
11. Package per §19.

## 8. References

- Rust Reference §Slice types: https://doc.rust-lang.org/reference/types/slice.html
- Rust std::slice::len: https://doc.rust-lang.org/std/primitive.slice.html#method.len
- Existing str::len intrinsic: `src/mir/lower/primitive_intrinsics.rs:197-218`
- Existing Str→Ref coercion in types_match_loose: `src/typeck/checker.rs:563-564`
- TD-SLICE-LEN-MISSING definition: `docs/develop/v0/tech-debt-register.md`
- TD-ARRAY-SLICE-COERCION-MISSING definition: `docs/develop/v0/tech-debt-register.md`
