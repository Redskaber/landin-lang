# Stage 3 Phase Gate Review — Round 13

> **Author**: redskaber
> **Date**: 2026-07-20
> **Process**: v3.12
> **Stage baseline**: v0.8.6 (Stage 3.45 — L10 float bitwise ops via cast)
> **Audit tool**: `examples/stage3_gate_audit_r13.rs`
> **Prior rounds**: R1-R12 all CONVERGED

---

## 1. Audit Design

R13 covers Stage 3.46 (L14 + L9 — full integer type support: i8 / i16 / i32 /
i64 / i128 / usize / isize, including correct LLVM IR types, arithmetic
instructions, overflow intrinsics, and shift-count overflow checks at the
correct bit widths).

30 cases across 3 groups:

| Group | Cases | Purpose |
|-------|-------|---------|
| R — Regression (8) | Re-verify Round 12 cases (const, enum, str, shift, float bitwise, overflow, struct, div-by-zero) |
| I — Stage 3.46 integer type coverage (14) | param types (i8/i16/u16/u32/usize/isize/i128), arithmetic bit width (i16/i128/usize), overflow intrinsics (i16/i128), shift overflow (i16/i128) |
| E — §9.3.2 edge cases (8) | i16 sub / i128 mul / i16 bitand / usize in if / i128 div-zero / mixed widths / i8 arith / usize shift |
| **Total** | **30** |

Per §9.3.1 (≥30 cases) and §9.3.2 (≥5 boundary cases) — both satisfied.

---

## 2. Audit Execution

```
✅ AUDIT PASSED — 0 codegen defects found in 30 cases.
   R1-R13: ..., 28, 28, 23, 24, 30/30 — all OK.
   Per §9.3.3, audit CONVERGED (13 rounds, 0 new issues each).
   Stage 3.46 (L14 + L9 full integer type support) verified.
```

---

## 3. Stage 3.46 Summary — Full Integer Type Support (L14 + L9)

### Problem

Before Stage 3.46, only `i32` and `i64` were first-class integer types in
codegen. Other widths were silently demoted or truncated:

| Landin type | Before 3.46 | After 3.46 |
|-------------|-------------|------------|
| `i8` / `u8` | `i8` (worked but no overflow intrinsic) | `i8` + `llvm.sadd.with.overflow.i8` |
| `i16` / `u16` | `i32` (wrong width — L14) | `i16` + `llvm.sadd.with.overflow.i16` |
| `i32` / `u32` | `i32` ✅ | `i32` ✅ (unchanged) |
| `i64` / `u64` | `i64` ✅ | `i64` ✅ (unchanged) |
| `i128` / `u128` | `i64` (truncated — L9) | `i128` + `llvm.sadd.with.overflow.i128` |
| `usize` / `isize` | `i32` (wrong on 64-bit) | `i64` (64-bit pointer width) |

Concretely:
- `fn f(a: i16, b: i16) -> i16 { a + b }` emitted `add nsw i32`, producing
  wrong-width arithmetic and a wrong-width overflow intrinsic.
- `fn f(a: i128, b: i128) -> i128 { a + b }` emitted `add nsw i64`, silently
  truncating 128-bit math to 64-bit.
- Shift-count overflow checks used the wrong bit width: `a << 2` on `i16`
  checked `icmp uge i32 2, 32` instead of `icmp uge i16 2, 16`.

### Root Cause (per §15 — root cause, not symptom)

1. `EmitType` only had `I32` / `I64` variants — no `I16` or `I128`.
2. `mir_type_to_emit_type` mapped `i16` → `I32` and `i128` → `I64` (silent
   demotion).
3. `binop_to_llvm_str` was a hardcoded match table with only `I32` / `I64`
   entries — adding new widths required touching every binop case.
4. `emit_checked_binop` only had `I32` / `I64` overflow intrinsic entries.
5. `detect_operand_type` looked only at the constant's *value kind*, not its
   *declared type* — so `1i16` was detected as `i32`.
6. `hir_ty_to_emit_type` had no `I16` / `I128` / `isize` / `usize` mappings.
7. Shift-count overflow check hardcoded the bit width per `EmitType` variant,
   so missing variants fell through to the default.

### Fix (per §15 — root cause, not symptom; per §17 — full type matrix coverage)

1. Added `EmitType::I16` and `EmitType::I128` variants.
2. `mir_type_to_emit_type`: `i16` / `u16` → `I16`; `i128` / `u128` → `I128`;
   `isize` / `usize` → `I64` (64-bit pointer width).
3. `binop_to_llvm_str`: **rewrote** from a hardcoded match table to a generic
   `format!("{} {}", op_str, ty_str)` that works for *all* integer widths
   (`i8` / `i16` / `i32` / `i64` / `i128`). Future widths need no code change.
4. `emit_checked_binop`: added `I8` / `I16` / `I128` overflow intrinsic
   entries (alongside existing `I32` / `I64`).
5. `detect_operand_type`: prefer the constant's *declared type* over its
   value-kind inference, so `1i16` is detected as `i16`.
6. `hir_ty_to_emit_type`: added `I16` / `I128` / `isize` / `usize` mappings.
7. Shift-count overflow: `I16` → 16 bits, `I128` → 128 bits, `I8` → 8 bits.

### Resulting IR

```llvm
; fn f(a: i16, b: i16) -> i16 { a + b }
define i16 @landin_f(i16 %arg0, i16 %arg1) {
  ...
  %v5 = call { i16, i1 } @llvm.sadd.with.overflow.i16(i16 %v3, i16 %v4)
  %v6 = extractvalue { i16, i1 } %v5, 0
  %v7 = extractvalue { i16, i1 } %v5, 1
  ; Assert(overflow)
  ...
  %v10 = add nsw i16 %v3, %v4
  ret i16 %v10
}

; fn f(a: i128) -> i128 { a << 2 }
define i128 @landin_f(i128 %arg0) {
  ...
  %v5 = icmp uge i128 2, 128    ; ← correct bit width
  br i1 %v5, label %panic, label %bb1
  ...
}
```

### Refactor Side-effect (per §15 — 最优 > 最小)

`src/codegen/mod.rs` was rewritten cleanly during this stage: the
`binop_to_llvm_str` match table was replaced by a generic width-parameterised
emitter. This removes a class of "forgot to add the new width" bugs going
forward and is the architectural-optimal choice rather than continuing to
patch the match table per-type.

---

## 4. Committee Vote: 5/5 APPROVED — UNANIMOUS

---

## 5. Updated Limitation List

| ID | Status |
|----|--------|
| L9 (i128) | **CLOSED in Stage 3.46** ✅ |
| L14 (i16/u16) | **CLOSED in Stage 3.46** ✅ |
| All prior CLOSED items (L2/L4/L6/L7/L10/L11/L12/L15/L-DEBT-2/L-MUT-1/L-DEBT-3/L-ENUM/L-ENUM-MATCH/L-CONST) | ✅ |
| Remaining open: L1 (PHI), L3 (closures), L5 (traits), L8 (lli), L13 (fat ptr), L-ENUM-UNION, L-COPY-ADT, L-PIPE-1 | Open |

---

## 6. Test Progression

| Version | Tests | Delta |
|---------|-------|-------|
| v0.8.6 (3.44, const/static) | 836 | +8 |
| v0.8.6 (3.45, L10 float bitwise) | 842 | +6 |
| **v0.8.6 (3.46, L14 + L9 full integer types)** | **855** | **+13** |

### New tests added in Stage 3.46 (13)

| Test | Asserts |
|------|---------|
| `codegen_i16_param` | `define i16 @landin_f(i16 %arg0)` |
| `codegen_u16_param` | `u16` → `i16` |
| `codegen_u32_param` | `u32` → `i32` |
| `codegen_usize_param` | `usize` → `i64` (64-bit) |
| `codegen_isize_param` | `isize` → `i64` (64-bit) |
| `codegen_i128_param` | `define i128 @landin_f(i128 %arg0)` |
| `codegen_i16_arith` | `add nsw i16` |
| `codegen_i128_arith` | `add nsw i128` |
| `codegen_usize_arith` | `add nsw i64` |
| `codegen_i16_overflow_check` | `llvm.sadd.with.overflow.i16` |
| `codegen_i128_overflow_check` | `llvm.sadd.with.overflow.i128` |
| `codegen_i16_shift_overflow` | `icmp uge … 16` |
| `codegen_i128_shift_overflow` | `icmp uge … 128` |

---

## 7. Audit Coverage Cross-check (per §17 — 测试矩阵全覆盖)

| Audit dimension | Cases | Source |
|-----------------|-------|--------|
| Param/return types | i01–i07 (7) | new in R13 |
| Arithmetic width | i08–i10 (3) | new in R13 |
| Overflow intrinsics | i11–i12 (2) | new in R13 |
| Shift overflow width | i13–i14 (2) | new in R13 |
| Edge: sub/mul/bitand/cmp/div/mixed/i8/usize-shift | e01–e08 (8) | new in R13 |
| Regression from R12 | r01–r08 (8) | carried forward |
| **Total** | **30** | ✅ ≥30 per §9.3.1 |

All Stage 3.46 codegen tests (`codegen_i16_*` … `codegen_i128_*`) are also
covered by the audit's `I` group, providing cross-verification between
`tests/v0/stage3/plan/codegen_tests.rs` and `examples/stage3_gate_audit_r13.rs` (per §17.2).

---

## 8. Conclusion

Stage 3 Round 13 **PASSED** with unanimous 5/5 approval. All 30 audit cases
pass, all 855 tests pass, 0 warnings.

L9 (i128) and L14 (i16/u16) **CLOSED**. Landin's codegen now supports the
**full set of Rust-like integer types** at their correct LLVM bit widths:

| Landin | LLVM | Width |
|--------|------|-------|
| `i8` / `u8` | `i8` | 8 |
| `i16` / `u16` | `i16` | 16 |
| `i32` / `u32` | `i32` | 32 |
| `i64` / `u64` | `i64` | 64 |
| `i128` / `u128` | `i128` | 128 |
| `isize` / `usize` | `i64` | 64 (target pointer width) |

Arithmetic, comparison, bitwise, shift, div-by-zero, and overflow checks all
emit at the correct width. No silent demotion or truncation remains.

**Remaining open limitations**: L1 (PHI optimization), L3 (closures),
L5 (trait dispatch), L8 (lli execution), L13 (fat pointers), L-ENUM-UNION,
L-COPY-ADT, L-PIPE-1.
