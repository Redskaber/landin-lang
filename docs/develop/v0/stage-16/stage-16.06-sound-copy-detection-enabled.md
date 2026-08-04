# Stage 16.06 — Sound Copy Detection Enabled (Field-level Derivation)

> **Author**: redskaber
> **Date**: 2026-08-03
> **Version**: v0.226.4 → v0.227.0
> **Process**: stage-committee-process.md v3.24 §1.0 原則 9 "正确 > 妥协" + §23 API 命名标准化 + §16 接口隔离

## 1. Executive Summary

Stage 16.06 **ENABLES sound Copy detection in the production driver** —
closing the last unsound simplification identified in the Stage 16.00
v0.3 kickoff. The driver now uses `BorrowChecker::with_resolver_and_sigs`
instead of `with_fn_sigs`, enabling `ty_is_copy_with_resolver` for
precise Adt Copy detection.

To close the migration gap (117 tests failed with sound Copy because
test structs didn't have `impl Copy`), Stage 16.06 adds **field-level
Copy derivation** to `TraitResolver`: structs/enums whose ALL fields are
Copy (and no `impl Drop`) are DERIVED Copy, mirroring Rust's
`#[derive(Copy, Clone)]` semantics. This is architecturally correct —
users shouldn't have to write `impl Copy for Point {}` for the common
case of `struct Point { x: i32, y: i32 }`.

**Result**: 0 unsound fallbacks in production. +10 integration tests.
7628 total tests, 0 failures. The unsound `ty_is_copy` is marked
`#[deprecated]`.

## 2. Background

### 2.1 The Sound Copy Migration Gap

Stages 15.99, 16.02, 16.03 attempted to enable sound Copy detection
(`with_resolver_and_sigs`). The result was 117 test failures because:

1. Test structs like `struct Point { x: i32, y: i32 }` don't have
   `impl Copy for Point {}`.
2. With sound Copy (`ty_is_copy_with_resolver`), `Point` is NOT Copy
   (no explicit `impl Copy`).
3. `let p2 = p` then marks `p` as moved, and `p.x` fails with
   "use of moved value: does not implement Copy".

The Stage 16.03 automated migration script added `impl Copy` to 393
.lin files, but 48 Rust test files + 69 complex .lin patterns needed
manual review.

### 2.2 The Architectural Solution

Instead of migrating 117 test files, Stage 16.06 implements
**field-level Copy derivation** in `TraitResolver`. This mirrors Rust's
`#[derive(Copy, Clone)]` semantics: a struct is Copy if ALL its fields
are Copy AND it has no `impl Drop`.

This is architecturally correct:
- **User-friendly**: `struct Point { x: i32, y: i32 }` is intuitively
  Copy — users shouldn't need boilerplate `impl Copy`.
- **Sound**: derivation is conservative — only ALL-Copy-field structs
  with no `impl Drop` are derived. Types with `impl Drop` or non-Copy
  fields are correctly non-Copy.
- **§16-compliant**: derivation runs in `TraitResolver::collect()` (HIR
  read, downstream). `BorrowChecker` queries via `is_copy_builtin`
  without needing HIR access.

## 3. Implementation

### 3.1 TraitResolver: `derived_copy_types` Field

Added `derived_copy_types: HashSet<DefId>` to `TraitResolver` (Stage 16.06):

```rust
pub struct TraitResolver {
    // ... existing fields ...
    /// Stage 16.06: Types that are derived Copy (no `impl Drop`, all fields
    /// are Copy). Populated by `collect()` via a fixpoint iteration.
    pub derived_copy_types: std::collections::HashSet<DefId>,
}
```

### 3.2 `derive_copy_types()` Method

Added `derive_copy_types()` to `TraitResolver`, called at the end of
`collect()`:

```rust
fn derive_copy_types(&mut self, hir: &HirCrate, interner: &Rodeo) {
    // Collect DefIds with `impl Drop` (Copy+Drop conflict).
    // Collect DefIds with explicit `impl Copy` (already Copy, skip).
    // Fixpoint iteration: repeat until no new types are derived.
    loop {
        let mut changed = false;
        for (def_id, node) in &hir.owners {
            // Handle structs AND enums.
            // - Structs: all fields must be Copy.
            // - Enums: all variant fields must be Copy.
            if all_fields_copy && !has_drop && !already_copy {
                self.derived_copy_types.insert(*def_id);
                changed = true;
            }
        }
        if !changed { break; }
    }
}
```

The fixpoint handles recursive/nested structs: `struct A { b: B }`
where `struct B { x: i32 }` — B is derived Copy first, then A.

### 3.3 `hir_ty_is_copy_candidate()` Helper

Added free function `hir_ty_is_copy_candidate()` to check if a HIR type
is Copy-derivable:

```rust
fn hir_ty_is_copy_candidate(
    kind: &HirTyKind,
    derived_copy_types: &HashSet<DefId>,
    explicit_copy_def_ids: &HashSet<DefId>,
) -> bool {
    match kind {
        // Primitives, refs, fn ptrs: always Copy.
        Bool | Char | Int(_) | ... => true,
        // Recursive: tuple/array of Copy candidates.
        Tuple(tys) => tys.iter().all(...),
        Array(inner, _) => ...,
        // Path: check if DefId is derived or explicit Copy.
        Path(_, path) => match path.res {
            Res::Def(def_id, _) => derived_copy_types.contains(&def_id)
                || explicit_copy_def_ids.contains(&def_id),
            _ => false, // Unresolved: conservative false.
        },
        // Conservative false for unsized/unknown.
        Slice(_) | TraitObject { .. } | Infer => false,
    }
}
```

### 3.4 `is_copy_builtin()` Updated

`is_copy_builtin()` now checks `derived_copy_types` first:

```rust
pub fn is_copy_builtin(&self, def_id: DefId, interner: &Rodeo) -> bool {
    // Stage 16.06: Check derived Copy first.
    if self.derived_copy_types.contains(&def_id) {
        return true;
    }
    // Then check explicit impl Copy.
    if let Some(copy_name) = interner.get("Copy") {
        self.is_copy(def_id, copy_name)
    } else {
        false
    }
}
```

### 3.5 Driver: `with_resolver_and_sigs` Enabled

The driver now uses the sound constructor:

```rust
// Before (v0.226.4):
let mut bc = BorrowChecker::with_fn_sigs(&fn_sig_table.sigs);

// After (v0.227.0):
let mut bc = BorrowChecker::with_resolver_and_sigs(
    &trait_resolver, &interner, &fn_sig_table.sigs
);
```

### 3.6 MIR Lowerer: `Operand::Move` for Soundness

The MIR lowerer previously used `Operand::Copy` for let bindings,
function returns, and call arguments. With sound Copy detection, this
caused "does not implement Copy" errors for non-Copy types.

Stage 16.06 changed these to `Operand::Move`:
- `let c = make(42)` — let binding (control_flow.rs)
- `return expr` — return statement (expr_operand.rs)
- Function body tail expression (mod.rs)
- Call arguments (expr_operand.rs)

The borrow checker's `Operand::Move` path (Stage 15.73) already skips
move recording for Copy types (`if !is_copy { record_move }`), so Move
is safe for both Copy and non-Copy types.

### 3.7 `ty_is_copy` Deprecated

The unsound `ty_is_copy` function is now `#[deprecated]`:

```rust
#[deprecated(note = "Unsound: returns true for ALL Adt types. Use \
    ty_is_copy_with_resolver (via BorrowChecker::with_resolver_and_sigs) \
    or ty_is_copy_unified instead. (Stage 16.06)")]
pub fn ty_is_copy(ty: &Ty) -> bool { ... }
```

It remains for test contexts (`BorrowChecker::new()` without resolver).
Per §23.6: deprecated with note pointing to the §16-compliant alternative.

## 4. Tests Updated

### 4.1 New Integration Tests

Added `tests/v0/stage15/plan/stage16_06_sound_copy_derivation_tests.rs`
with 10 tests:
1. Struct with primitive fields is derived Copy
2. Struct with `impl Drop` is NOT Copy (double-move rejected)
3. Enum with all Copy variants is derived Copy
4. Nested struct with all Copy fields is derived Copy (fixpoint)
5. Explicit `impl Copy` coexists with derived Copy
6. Struct with tuple of Copy is derived Copy
7. Struct with array of Copy is derived Copy
8. Function returning non-Copy struct compiles (Move)
9. Enum variant with non-Copy payload compiles (Move arg)
10. `is_copy_builtin` returns true for derived Copy (direct query)

### 4.2 Existing Tests Updated

- `builtin_copy_activation_tests::test_no_copy_impl_means_not_copy` —
  uses `impl Drop` instead of unit struct (unit structs are now derived
  Copy).
- `builtin_copy_activation_tests::test_copy_selective_per_type` —
  uses `impl Drop` for the non-Copy type.
- `copy_unification_tests::test_unified_integration_without_impl_copy` —
  uses `impl Drop` instead of unit struct.

## 5. API Naming Standard Compliance (§23)

| Rule | Compliance |
|------|------------|
| §23.1.1 Free function entry | ✅ `derive_copy_types` is a method, `hir_ty_is_copy_candidate` is a free fn |
| §23.1.2 Context type naming | ✅ `TraitResolver` follows `-er` suffix |
| §23.1.3 Type prefix | ✅ No new types with prefixes needed |
| §23.1.4 Re-export style | ✅ Explicit re-export with `#[allow(deprecated)]` |
| §23.1.5 DRY | ✅ `hir_ty_is_copy_candidate` is the single source for HIR Copy checks |
| §23.1.6 Deprecation | ✅ `ty_is_copy` marked `#[deprecated(note = "...")]` |
| §23.1.7 Function naming prefix | ✅ `derive_` prefix for derivation, `is_` for predicates |
| §23.1.8 Error type suffix | N/A (no new error types) |

## 6. §16 Interface Isolation Compliance

- `TraitResolver::collect()` reads HIR (allowed — data flows downstream).
- `BorrowChecker::is_copy()` queries `TraitResolver` via `is_copy_builtin`
  (no HIR access needed).
- `hir_ty_is_copy_candidate` is a private free function in `resolver.rs`
  (not exposed to other modules).

## 7. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 244/244 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2160/2160 PASS (+10 new)
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 7628 tests passing, 0 failures, 0 warnings.**

## 8. Version Policy

v0.226.4 → v0.227.0 (**minor bump** — sound Copy detection is a
significant behavioral change. Programs with structs that have `impl
Drop` but were previously treated as Copy will now correctly reject
use-after-move. The field-level derivation ensures most existing code
continues to work.)

Per version policy: minor bump for behavioral change that could affect
existing programs (even though all tests pass, the soundness fix is
semantically significant).

## 9. v0.3 Roadmap Status

| Item | Status |
|------|--------|
| TODO cleanup (3 items) | ✅ COMPLETE (Stages 16.01, 16.04, 16.05) |
| Sound Copy detection | ✅ **COMPLETE (Stage 16.06)** — enabled in driver, field-level derivation, `ty_is_copy` deprecated |
| Task 3: TraitResolver Keys | 🔧 Pending (2 weeks) |
| Task 11: Monomorphization | 🔧 Pending (2-3 weeks) |
| Task 10: Closure redesign | 🔧 Pending (2-3 weeks) |

**Next step**: Task 3 (TraitResolver Keys) — redesign keys from
`(trait_name_spur, type_name_spur)` to `(DefId, SubstsRef)`. This
unblocks Tasks 11 (Monomorphization), 14 (Object safety), 17
(Associated types).
