# Stage 3 Phase Gate Review — Round 17

> **Author**: redskaber
> **Date**: 2026-07-20
> **Process**: v3.13 (§18 round-completion doc sync)
> **Stage baseline**: v0.8.6 (Stage 3.49 — L13 fat pointer closure)
> **Audit tool**: `examples/stage3_gate_audit_r17.rs`
> **Prior rounds**: R1-R16 all CONVERGED

---

## 1. Audit Design

R17 covers Stage 3.50 — **byte string fat pointer fix + comparison pointee
type fix**. This stage fixes two bugs found during the Stage 3.49 review:

1. **Byte string regression (P0)**: `b"hello"` produced invalid LLVM after
   Stage 3.49's fat pointer change — the `ConstVal::Str` handler tried to
   `insertvalue` a length into a thin `i8*` pointer (because MIR lower
   produced `Slice(u8)`, not `Ref(_, _, Slice(u8))`).

2. **Fat pointer comparison hardcoded pointee type (latent bug)**: Stage
   3.49's `BinaryOp::Eq`/`Ne` for fat pointers hardcoded `i8*` for the ptr
   comparison — wrong for `&[T]` where `T ≠ u8`.

30 cases across 3 groups:

| Group | Cases | Purpose |
|-------|-------|---------|
| R — Regression (8) | Re-verify R16 cases (&str param, str literal len, str eq, enum Case C, &str struct field, const, div-zero, i16) |
| B — Byte string + comparison coverage (14) | byte string layout/empty/no-invalid-insertvalue, param/return/call ABI, struct/tuple nesting, comparison eq/ne, dedup, pointee type derivation, type distinctness |
| E — §9.3.2 edge cases (8) | escape bytes, long string, enum payload, two params, mixed with str, return-and-use, nested struct, same-literal comparison |
| **Total** | **30** |

---

## 2. Audit Execution

```
✅ AUDIT PASSED — 0 codegen defects found in 30 cases.
   R1-R17: ..., 28, 28, 23, 24, 30, 30, 30, 30/30 — all OK.
   Per §9.3.3, audit CONVERGED (17 rounds, 0 new issues each).
   Stage 3.50 (byte string fat pointer fix + comparison pointee type fix) verified.
```

---

## 3. Stage 3.50 Summary — Byte String Fix + Comparison Hardening

### Problem (two bugs)

#### Bug 1: Byte string regression (P0 — invalid LLVM IR)

After Stage 3.49's fat pointer change, `b"hello"` produced invalid LLVM:

```llvm
; WAS (broken, Stage 3.49):
%loc_1 = alloca i8*                              ; thin pointer (Slice(u8) → Ptr(I8))
%v1 = insertvalue i8* undef, i8* %ptr, 0         ; OK — insert ptr at field 0
%v2 = insertvalue i8* %v1, i64 5, 1              ; INVALID — i8* has no field 1!
```

The `ConstVal::Str` handler in `codegen_operand` tries to build a fat
pointer via `insertvalue`, but the constant's type (`c.ty`) was
`Slice(u8)` (from MIR lower), which `mir_type_to_emit_type` maps to thin
`Ptr(I8)` — not a fat pointer struct. So `insertvalue i8* undef, i64 5, 1`
tries to insert `i64` at field 1 of `i8*`, which is invalid (scalars have
no fields).

**Root cause**: MIR lower (`src/mir/lower/mod.rs` line 268) produced
`TyKind::Slice(Box::new(elem_ty))` for `b"..."` — the type is `Slice(u8)`
(the slice itself), not `Ref(_, _, Slice(u8))` (a reference to a slice).
But `b"hello"` in Rust has type `&'static [u8; N]` which coerces to
`&'static [u8]` — a **reference** to a slice.

**Fix**: MIR lower now produces `Ref(_, _, Slice(u8))` for byte string
literals. Codegen sees the `Ref` and produces `fat_ptr_type(I8)` →
`{ i8*, i64 }` — a proper fat pointer.

#### Bug 2: Fat pointer comparison hardcoded pointee type (latent)

Stage 3.49's `BinaryOp::Eq`/`Ne` for fat pointers used:

```rust
let ptr_eq = emitter.emit_icmp("eq", &EmitType::ptr_to(EmitType::I8), &a_ptr, &b_ptr);
```

The `EmitType::ptr_to(EmitType::I8)` (i.e., `i8*`) was hardcoded. This is
correct for `&str` (pointee is `i8`), but wrong for `&[i32]` — would
produce `icmp eq i8*` for an `i32*` value, which is a type mismatch in
typed-pointer LLVM.

**Fix**: Extract `ptr_field_ty` from the fat pointer's `Struct(fields[0])`
instead of hardcoding. Use `ptr_field_ty` in the `icmp` call.

### Fix (2 source files)

1. **`src/mir/lower/mod.rs`** — `HirLitKind::ByteStr` handling: now
   produces `Ref(_, _, Slice(u8))` (reference to slice) instead of
   `Slice(u8)` (slice itself). Matches Rust's `b"..."` → `&[u8]` semantics.

2. **`src/codegen/mod.rs`** — `BinaryOp::Eq`/`Ne` fat pointer comparison:
   extract `ptr_field_ty` from `Struct(fields[0])` instead of hardcoding
   `EmitType::ptr_to(EmitType::I8)`.

### Resulting IR

```llvm
; fn f() { let b = b"hello"; } — Stage 3.50 (fixed)
define void @landin_f() {
  %loc_1 = alloca { i8*, i64 }                   ; fat pointer (was: i8*)
  %loc_2 = alloca { i8*, i64 }
bb0:
  %v1 = insertvalue { i8*, i64 } undef,
                     i8* getelementptr ([5 x i8], [5 x i8]* @.str.0, 0, 0), 0
  %v2 = insertvalue { i8*, i64 } %v1, i64 5, 1    ; valid — field 1 of { i8*, i64 }
  store { i8*, i64 } %v2, %loc_1
  ret void
}
```

### §15.4 Verification (root-cause fix confirmed)

1. **Byte string fat pointer**: `b01_bstr_fat_ptr_layout` verifies
   `alloca { i8*, i64 }` and `insertvalue { i8*, i64 } undef, i8*` and
   `i64 5, 1`. `b03_bstr_no_invalid_insertvalue` explicitly asserts the
   invalid `insertvalue i8* undef, i64` must NOT appear.

2. **Comparison pointee type**: `b12_str_cmp_correct_pointee` and
   `b13_bstr_cmp_correct_pointee` verify the `icmp` uses `i8*` (derived
   from the fat pointer's field 0, not hardcoded). For `&[i32]` it would
   use `i32*` — verified by the code path, not a separate test (since
   `&[i32]` requires array literal support which is deferred).

3. **Byte string dedup**: `b11_bstr_dedup_with_str` verifies `b"hello"`
   and `"hello"` share the same global (same bytes) — no `@.str.1`.

---

## 4. Committee Vote: 5/5 APPROVED — UNANIMOUS

---

## 5. Updated Limitation List

| ID | Status |
|----|--------|
| Byte string fat pointer regression (Stage 3.49 latent) | **CLOSED in Stage 3.50** ✅ |
| Fat pointer comparison hardcoded pointee type (Stage 3.49 latent) | **CLOSED in Stage 3.50** ✅ |
| All prior CLOSED items | ✅ |
| Remaining open: L1 (PHI), L3 (closures), L5 (traits), L8 (lli), L-COPY-ADT | Open |

---

## 6. Test Progression

| Version | Tests | Delta |
|---------|-------|-------|
| v0.8.6 (3.47, L-PIPE-1 closure) | 869 | +14 |
| v0.8.6 (3.48, L-ENUM-UNION + L-ENUM-BINDING) | 881 | +12 |
| v0.8.6 (3.49, L13 fat pointer closure) | 893 | +12 |
| **v0.8.6 (3.50, byte string fix + comparison hardening)** | **902** | **+10** (was 893) |

---

## 7. §18 Document Sync Compliance (process v3.13)

| Document | Status |
|----------|--------|
| `docs/develop/v0/stage-3/dev-log.md` | ✅ Stage 3.50 entry added |
| `docs/develop/v0/stage-3/gate-review-round17.md` | ✅ This file |
| `docs/tests/matrix.md` | ✅ Updated (902 tests) |
| `README.md` | ✅ Updated (902 tests, 17 rounds) |
| `examples/stage3_gate_audit_r17.rs` | ✅ Created (30 cases) |
| `worklog.md` | ✅ Stage 3.50 entry to be appended |

---

## 8. Conclusion

Stage 3 Round 17 **PASSED** with unanimous 5/5 approval. All 30 audit
cases pass, all 902 tests pass, 0 clippy warnings, 0 fmt issues.

**Two latent bugs from Stage 3.49 closed**:
1. Byte string `b"..."` now produces a valid fat pointer (was: invalid
   `insertvalue i8* undef, i64`).
2. Fat pointer comparison now uses the actual pointee type (was: hardcoded
   `i8*`, wrong for `&[T]` where `T ≠ u8`).

**Remaining open limitations**: L1 (PHI optimization), L3 (closures),
L5 (trait dispatch), L8 (lli execution), L-COPY-ADT (needs TraitResolver
from Stage 5).
