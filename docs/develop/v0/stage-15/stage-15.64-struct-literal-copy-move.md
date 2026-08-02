# Stage 15.64 — Struct Literal Copy→Move + Field-Copy Drop Prevention

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.189.0 → v0.190.0
> **Process**: stage-committee-process.md v3.23 §29
> **v0.2 Phase 3 Task 13**: `impl Drop` + RAII — struct literal + field access fix

## 1. Executive Summary

Stage 15.64 fixes two root causes of extra drops (double-drop of temporaries)
when using `impl Drop` types in struct literals and field accesses:

1. **Struct literal Copy→Move**: When a struct literal has a field whose
   type is non-Copy (e.g., a struct with `impl Drop`), the field value is
   now **moved** (not copied) into the struct. Previously, `Operand::Copy`
   was used for ALL field values, causing the field's temporary to not be
   marked as moved → `elaborate_drops` inserted a Drop for it → double-drop.

2. **Field-copy drop prevention**: When a field of a struct is accessed
   (e.g., `o.inner`), the intermediate temp that holds the field value
   is now **excluded from drop**. Previously, the temp was dropped at scope
   end, causing a double-drop (the field is also dropped when the struct
   is dropped via recursive drop glue).

**Runtime verification** — the program:
```landin
trait Drop { fn drop(&mut self); }
struct Inner { x: i32 }
impl Drop for Inner { fn drop(&mut self) { println!("inner dropped") } }
struct Outer { inner: Inner }
impl Drop for Outer { fn drop(&mut self) { println!("outer dropped") } }
fn main() -> i32 {
    let o = Outer { inner: Inner { x: 42 } };
    o.inner.x
}
```

**Before Stage 15.64** (4 drops — 2 extra):
```
inner dropped   ← extra (field-copy temp)
outer dropped   ← correct
inner dropped   ← correct (recursive field drop)
inner dropped   ← extra (struct literal temp)
```

**After Stage 15.64** (2 drops — correct):
```
outer dropped   ← o's user drop
inner dropped   ← o's recursive field drop
```

This matches Rust's semantics exactly.

## 2. Root Cause Analysis

### 2.1 Bug #1: Struct literal always uses `Operand::Copy`

In `src/mir/lower/expr_operand.rs`, the `HirExprKind::Struct` arm used
`Operand::Copy` for ALL field values:

```rust
let operands: Vec<Operand> = field_locals
    .iter()
    .map(|l| Operand::Copy(Place::local(*l, Span::DUMMY)))  // ← always Copy
    .collect();
```

For non-Copy field types (e.g., `Inner` with `impl Drop`), this is wrong:
- The field temp is NOT marked as moved (Copy doesn't record moves).
- `elaborate_drops` sees the temp's `StorageDead` and inserts a Drop.
- The struct itself also drops the field (via recursive drop glue).
- Result: double-drop of the Inner value.

**Fix**: Check the field type's Copy-ness. Use `Operand::Copy` for Copy
types, `Operand::Move` for non-Copy types. Uses the new shared
`is_mir_ty_copy_conservative` helper from `mir::ty`.

### 2.2 Bug #2: Field access temp is dropped

In `src/mir/lower/expr_operand.rs`, the `HirExprKind::Field` arm creates
an intermediate temp:

```rust
let result = cx.mir.new_local(field_ty, None, expr.span);
cx.push_assign(
    Place::local(result, expr.span),
    Rvalue::Use(Operand::Copy(Place {  // ← always Copy
        kind: PlaceKind::Projection(base_place, ProjectionElem::Field(...)),
        ...
    })),
    expr.span,
);
```

For `o.inner.x`, this produces:
```text
temp5 = Use(Copy(Projection(Local(o), Field(0))))  // copies o.inner → temp5
temp6 = Use(Copy(Projection(temp5, Field(0))))     // copies temp5.x → temp6
```

`temp5` has type `Inner` (non-Copy, needs drop). Since it's not moved,
`elaborate_drops` inserts a Drop for it. But `o` also drops `inner` via
recursive drop glue → double-drop.

**Fix**: Added `collect_field_copy_locals` function that finds locals
assigned from `Use(Copy(Projection(...)))` and adds them to the skip-drop
set in `elaborate_drops`. These temps hold a "view" of the field, not an
owned value — the original struct owns the field.

## 3. What Was Done

### 3.1 Added `is_mir_ty_copy_conservative` to `mir::ty`

New shared function in `src/mir/ty.rs`:
- Returns `true` for types that are ALWAYS Copy (primitives, refs, fn types).
- Returns `false` for types that MAY or MAY NOT be Copy (Adt, Str, Slice, etc.).
- Recursively checks Tuple and Array.
- Conservative: treats Adt as non-Copy (sound — avoids false Copy).

Per §23 rule 5 (DRY): replaces inline checks in `control_flow.rs` (let
bindings) and `expr_operand.rs` (struct literals).

### 3.2 Fixed struct literal to use Move for non-Copy fields

In `src/mir/lower/expr_operand.rs`, the `HirExprKind::Struct` arm now:
```rust
let operands: Vec<Operand> = field_locals
    .iter()
    .map(|l| {
        let field_ty = &cx.mir.local(*l).ty;
        if crate::mir::ty::is_mir_ty_copy_conservative(field_ty) {
            Operand::Copy(Place::local(*l, Span::DUMMY))
        } else {
            Operand::Move(Place::local(*l, Span::DUMMY))
        }
    })
    .collect();
```

### 3.3 Updated `let` binding to use shared helper (DRY)

In `src/mir/lower/control_flow.rs`, replaced the inline Copy check with:
```rust
let is_copy = crate::mir::ty::is_mir_ty_copy_conservative(&init_ty);
```

### 3.4 Added `collect_field_copy_locals` to drop elaboration

New function in `src/mir/drop_elaboration.rs`:
- Scans all Assign statements for `dest = Use(Copy(Projection(base, Field(...))))`.
- Collects the `dest` local IDs.
- `elaborate_drops` adds these to the skip-drop set (union with moved locals).

## 4. Verification

### 4.1 Quality checks
- `cargo clean && cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings

### 4.2 Test results
- `cargo test --features llvm-backend --lib` — ✅ 226/226 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2126/2126 PASS
  (was 2118; +8 new struct literal tests, 2 ignored)
- `python3 tests/conformance/run_all.py` — ✅ 5216/5216 PASS

**Total: 7568 tests passing, 0 failures, 0 warnings.**

### 4.3 Runtime verification

The test program (above) now produces exactly 2 drops (correct):
```
outer dropped
inner dropped
```

## 5. Files Modified

### 5.1 `src/mir/ty.rs`
- **Lines 266-315** (NEW): Added `is_mir_ty_copy_conservative` function.

### 5.2 `src/mir/lower/expr_operand.rs`
- **Lines 1742-1766**: Struct literal now uses Move for non-Copy fields.

### 5.3 `src/mir/lower/control_flow.rs`
- **Lines 618-623**: `let` binding now uses shared `is_mir_ty_copy_conservative`.

### 5.4 `src/mir/drop_elaboration.rs`
- **Lines 115-157** (NEW): Added `collect_field_copy_locals` function.
- **Lines 395-410**: `elaborate_drops` now unions moved + field-copy locals.
- **Line 42**: Added `ProjectionElem` import.

### 5.5 `tests/v0/stage15/plan/struct_literal_copy_move_tests.rs` (NEW)
- 8 integration tests covering struct literal Copy→Move and field-copy prevention.

### 5.6 `tests/all_tests.rs`
- Registered `stage15_struct_literal_copy_move_tests` module.

### 5.7 `Cargo.toml`
- Bumped v0.189.0 → v0.190.0.

## 6. §23 API Naming Standardization Audit

- ✅ `is_mir_ty_copy_conservative` — `<noun>_<noun>_<adjective>` (rule 1 spirit).
- ✅ `collect_field_copy_locals` — `<verb>_<noun>_<noun>` (rule 1, matches
  existing `collect_moved_locals` pattern).
- ✅ No new types introduced (rules 2-3 N/A).
- ✅ No new re-exports (rule 4 N/A).
- ✅ DRY: `is_mir_ty_copy_conservative` replaces 2 inline checks (rule 5).

## 7. §25 Deep Review (8 Dimensions)

### D1. Architecture Health — ✅ Excellent
- Shared `is_mir_ty_copy_conservative` in `mir::ty` (neutral location, §16).
- `collect_field_copy_locals` in drop_elaboration (same module as
  `collect_moved_locals` — consistent pattern).

### D2. Technical Debt — ✅ Good (improved)
- Struct literal Copy→Move: **RESOLVED**.
- Field-copy temp double-drop: **RESOLVED**.
- Remaining: full drop flags (runtime tracking) — P2, v0.3.

### D3. Test Coverage — ✅ Excellent
- 8 new integration tests covering all patterns.
- Runtime verification: correct drop count (2, not 4).
- All 5216 conformance tests pass (no regression).

### D4-D8 — ✅ All Excellent
(Same rationale as Stage 15.63.)

## 8. Committee Vote: GO

**Decision**: Stage 15.64 is **COMPLETE**. Struct literal and field access
now produce correct drop behavior — no extra drops.

## 9. Remaining Work (Deferred to v0.3)

| Item | Effort | Priority |
|------|--------|----------|
| Full drop flags (runtime tracking) | 2-3 days | P2 |
| Nested Projection places (no intermediate temps) | 2-3 days | P2 |
| Partial move handling | 1 day | P2 |
| `Box<T>` in prelude | 2 days | P2 |
