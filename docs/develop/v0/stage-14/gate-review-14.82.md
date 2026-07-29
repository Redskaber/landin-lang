# Stage 14.82 — Gate Review

> **Author**: redskaber
> **Date**: 2026-07-30
> **Version**: v0.97.0 → v0.98.0
> **Process**: stage-committee-process.md v3.22 §25 (D8 review)

## 1. Stage Summary

Stage 14.82 partially fixes **GAP-7 (disjoint closure captures — RFC 2229)**.
The most critical manifestation — closures capturing structs crashing LLVM
verification — is now fixed. True disjoint field-level captures remain
deferred to Stage 14.83+.

## 2. Bug Fixed

### GAP-7 (partial): Closure capturing struct crashed LLVM verification

**Symptom**: `let f = || p.x;` where `p` is a `Point { x: i32, y: i32 }`
struct failed with:

```
Invalid InsertValueInst operands!
  %v9 = insertvalue { i32 } undef, { i32, i32 } %v8, 0
```

The closure struct was typed `{ i32 }` (one i32 field) but the operand was
a `{ i32, i32 }` (Point struct) value.

**Root cause** (3 layers):

1. **`src/codegen/emitter.rs` `TyKind::Closure` arm** (line 506-508): used
   `mir_type_to_emit_type` (legacy, no layouts) for substs — falling back
   to `EmitType::I32` for any `Adt` capture.

2. **`src/codegen/mir_translation.rs` `mir_type_to_emit_type_with_layouts`**:
   no explicit `Closure` arm — fell through to the legacy variant (which
   has the bug from #1).

3. **`src/driver.rs`**: closure substs captured `p`'s MIR type BEFORE
   typeck ran. The MIR-lower produced `Closure(_, [Infer(TyVar)])` because
   `p`'s type was `Infer` at MIR-lower time. After typeck wrote back
   `Adt(Point)` to `p`'s `local_decl.ty`, the closure's substs still held
   the stale `Infer(TyVar)`.

**Fix** (4 changes):

1. `src/codegen/mir_translation.rs`: Added explicit `TyKind::Closure` arm
   to `mir_type_to_emit_type_with_layouts` that recurses with layouts.

2. `src/codegen/rvalue.rs` `AggregateKind::Closure` codegen: Use
   `mir_type_to_emit_type_with_layouts` (was: legacy variant) for
   `field_tys`.

3. `src/mir/lower/adt_layout.rs`: `collect_adt_def_ids` recurses into
   `Closure` substs; `populate_adt_layouts` walks
   `Aggregate(Closure, ...)` rvalues' substs.

4. `src/driver.rs`: After typeck, walk every `Aggregate(Closure, operands)`
   rvalue and write back each operand's source local resolved type to the
   corresponding subst. Then re-run `populate_adt_layouts` to register any
   newly-exposed Adt captures.

## 3. Verification

- `let f = || p.x;` with `p = Point { x: 10, y: 20 }` → outputs `10` ✅
- All 1951 rust tests pass (zero regression)
- All 5171 conformance tests pass (was 5170, +1 new run_ok test)
- 0 clippy warnings, fmt clean

## 4. Known limitation deferred to Stage 14.83+

True disjoint field-level captures (RFC 2229) are NOT implemented:

```rust
let p = Point { x: 10, y: 20 };
let f = || p.x;  // should capture only p.x (i32), not whole p
let g = || p.y;  // should capture only p.y (i32), not whole p
```

Currently both closures capture the whole `p` struct. For the simple case
(`let f = || p.x;` alone), this works correctly. For the disjoint case
(both `f` and `g`), the second closure's `p.y` access reads from a
different location, producing garbage output.

This is a deeper change — requires `closure_capture.rs::collect_captured_locals`
to track field-level captures (e.g., `CapturedPlace::Field(p, 0)` instead of
`CapturedPlace::Local(p)`). Deferred to Stage 14.83+ as a separate P1 fix.

## 5. P0 Blockers Status

| ID | Status |
|----|--------|
| GAP-1 | ✅ FIXED (Stage 14.81) |
| GAP-2 | Pending (L3) |
| GAP-3 | Pending (L3) |
| GAP-4 | Pending (L2, low priority) |
| GAP-5 | ✅ Already working (Stage 14.81) |
| GAP-6 | ✅ Already working (Stage 14.81) |
| **GAP-7** | **⚠️ Partial fix (Stage 14.82)** — struct captures work; disjoint field captures deferred |

## 6. Design Doc Alignment (§13.4)

No new design doc deviations. The fix is consistent with
`04-ownership-borrowing.md` §4 (closure captures) — the spec was correct,
the implementation had multiple bugs.

## 7. Next Stage Plan

- **Stage 14.83**: GAP-7 disjoint field-level captures (RFC 2229) — P1
- **Stage 14.84+**: API naming standardization audit (§23)
- **Deferred past v0.1**: GAP-2/3/4 (L3 infrastructure work)
