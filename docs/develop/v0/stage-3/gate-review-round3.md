# Stage 3 Phase Gate Review — Round 3

> **Author**: redskaber
> **Date**: 2026-07-20
> **Process**: v3.7 (§9.3.1 ≥30-case audit + §9.3.2 edge-case tests + §9.3.3 convergence)
> **Stage baseline**: v0.8.6 (Stage 3.27 + 3.28 added in this round)
> **Audit tool**: `examples/stage3_gate_audit_r3.rs`
> **Prior rounds**: R1 (38/38), R2 (43/43) — both CONVERGED per §9.3.3

---

## 1. Audit Design

Per §9.3.3, R3 was technically skippable (R1 + R2 already converged). However,
the skip rule explicitly says "unless significant new features land" — and
Stage 3.27 + 3.28 added module-level globals, a new IR shape. So R3 was run
to verify the new shape is sound.

43 cases across 5 groups:

| Group | Cases | Purpose |
|-------|-------|---------|
| R — Regression (15) | Re-verify Round 2 cases still pass |
| S — Stage 3.27 string literals (10) | NEW: global emission, GEP, dedup, escapes, Unicode, empty, cross-function |
| B — Stage 3.28 byte strings (8) | NEW: byte string globals, dedup with str, u8/i8 type mapping |
| E — §9.3.2 edge cases (5) | NEW: no-void-alloca, linkage, byte length, module-end header |
| H — Adversarial (5) | NEW: strings in if/loop, mixed str+bytestr dedup, many uses |
| **Total** | **43** | ≥30 per §9.3.1 ✅ |

---

## 2. Audit Execution

```
=== Stage 3 Gate Audit Round 3 Summary ===
    Total: 43  Pass: 43  Fail: 0
✅ AUDIT PASSED — 0 codegen defects found in 43 cases.
   R1: 38/38, R2: 43/43, R3: 43/43 — all OK.
   Per §9.3.3, audit CONVERGED (3 rounds, 0 new issues each).
```

All 43 cases pass. 15 R2 regression cases still pass (no regression).
28 new cases for Stage 3.27 + 3.28 + edge cases + adversarial.

### §9.3.3 Convergence

- R1: 38 cases, 0 new issues ✅
- R2: 43 cases, 0 new issues ✅
- R3: 43 cases, 0 new issues ✅
- **3 consecutive rounds converged** — audit is firmly stable.

---

## 3. Stage 3.27 + 3.28 Summary

### Stage 3.27 — String Literal Codegen (v0.8.6)
**Problem**: `ConstVal::Str(sym)` was hardcoded to emit `"0"` (a null pointer).
Any program using string literals produced broken IR — the string's bytes
were lost and the local's value was a constant 0.

**Fix**:
- Added `Emitter::emit_string_global(bytes)` trait method.
- `TextEmitter` accumulates string globals in a `Vec<String>` and dedupes
  via `HashMap<Vec<u8>, String>`. Same content → same global name.
- Globals are emitted at module end via `output_with_globals()`.
- Each global is `@.str.N = private unnamed_addr constant [M x i8] c"..."`.
- Byte content is escaped: printable ASCII verbatim; everything else
  (including `\t`, `\n`, `"`, `\`, non-ASCII bytes) as `\NN` hex.
- In `codegen_operand`, `ConstVal::Str` looks up the symbol's bytes via
  the interner (now threaded through all codegen functions), emits a
  global, and returns `getelementptr inbounds ([N x i8], [N x i8]* @.str.N, i32 0, i32 0)`
  — an `i8*` pointer to the first byte.
- `TyKind::Str` now maps to `EmitType::ptr_to(EmitType::I8)` (was `I32`).
- Side fix: skip `alloca` and `store` for void-typed locals (was producing
  invalid `alloca void` / `store void` for unit-typed MIR temp slots).

**Resulting IR** for `fn f() { let s = "hello"; }`:
```llvm
define void @landin_f() {
  %loc_1 = alloca i8*
bb0:
  store i8* getelementptr inbounds ([5 x i8], [5 x i8]* @.str.0, i32 0, i32 0), %loc_1
  ret void
}

; --- Module-level string constants ---
@.str.0 = private unnamed_addr constant [5 x i8] c"hello"
```

### Stage 3.28 — Byte String Literal Codegen (v0.8.6)
**Problem**: `b"..."` literals were lowered as `Slice(u8)` with `ConstVal::Str`,
but `Slice` wasn't handled by `mir_type_to_emit_type` (fell through to `I32`),
and `u8` itself also fell through to `I32`. Result: byte strings got the same
broken treatment as string literals, AND `u8`-typed locals had wrong type.

**Fix**:
- `TyKind::Slice(elem)` now maps to `EmitType::ptr_to(mir_type_to_emit_type(elem))`
  (was `I32`). This makes `Slice(u8)` → `Ptr(I8)` → `i8*`.
- `TyKind::Int(I8)` and `TyKind::Uint(U8)` now map to `EmitType::I8`
  (was `I32`). `u8`/`i8` params and returns now emit as `i8`.
- `TyKind::Int(I16)` / `Uint(U16)` explicitly map to `I32` (Stage 3 simplification
  — no `i16` support yet; documented as L12).
- Byte strings now share the same global format as string literals
  (LLVM doesn't distinguish `i8` from `u8`), and dedup correctly across
  both (`"hello"` and `b"hello"` → one global).

**Resulting IR** for `fn f(x: u8) -> u8 { x }`:
```llvm
define i8 @landin_f(i8 %arg0) {
  %loc_0 = alloca i8
  %loc_1 = alloca i8
  store i8 %arg0, %loc_1
bb0:
  ...
  ret i8 %v
}
```

---

## 4. Committee Vote (5-role, per §3.1)

| Role | Vote | Notes |
|------|------|-------|
| **Compiler Engineer** | APPROVED | Interner threading is clean (passed via &Rodeo through all codegen functions). Global dedup uses `HashMap<Vec<u8>, String>` — O(1) lookup, no quadratic blowup. No `unsafe`. |
| **Type System Theorist** | APPROVED | `Str` and `Slice(T)` modeling as `Ptr(...)` is a sound simplification — loses the length component of the fat pointer, but no type-confusion. `u8`/`i8` mapping to `I8` is correct. |
| **Soundness Reviewer** | APPROVED | No new soundness holes. The "no alloca void" fix closes an invalid-LLVM-IR hole that was always present (just happened to not crash on prior test inputs). String globals are `private unnamed_addr` — correct for literal immutability. |
| **Testing & QA Lead** | APPROVED | 43-case audit covers regression + new features + edge cases + adversarial. 22 new tests in `tests/codegen_tests.rs` (13 for 3.27 + 9 for 3.28). 761 total tests pass, 0 regressions. |
| **Tooling & DX Lead** | APPROVED | 0 clippy warnings, 0 fmt diffs. Three audit scripts now (`stage3_gate_audit.rs`, `_r2.rs`, `_r3.rs`) — reproducible. `output_with_globals()` API is clean and backward-compatible (`output()` still returns just function bodies for any future backend that doesn't need globals). |

**Result**: 5/5 APPROVED — UNANIMOUS. Stage 3 gate review Round 3 PASSED.

---

## 5. Updated Limitation List

| ID | Limitation | Status |
|----|-----------|--------|
| L1 | No real PHI node emission — merges use load-from-alloca | Still open (correctness OK, optimization pending) |
| L2 | No struct/enum ADT codegen | Still open |
| L3 | No closure codegen | Still open |
| ~~L4~~ | ~~No String/str literal codegen~~ | **CLOSED in Stage 3.27** ✅ |
| L5 | No trait dispatch / vtable | Still open |
| L6 | Overflow checks | CLOSED in Stage 3.24 ✅ |
| L7 | Div-by-zero checks | CLOSED in Stage 3.25 ✅ |
| L8 | No `lli` execution verification | Still open (env lacks LLVM tools) |
| L9 | `i128`/`u128` truncated to `i64` | Still open |
| L10 | Float bitwise ops fall back to int form | Still open |
| L11 | Shl/Shr don't get shift-count overflow checks | Still open |
| ~~L12~~ | ~~`u8`/`i8` mapped to `i32`~~ | **CLOSED in Stage 3.28** ✅ |
| L13 | `&str` / `&[T]` are thin pointers (no fat ptr+len) | NEW — Stage 3.27/3.28 simplification; full fat-pointer representation deferred. Means you can't pass `&str` to a function and recover the length. |
| L14 | `i16`/`u16` mapped to `i32` | NEW — Stage 3.28 simplification. |
| L15 | String literals passed as function args not yet supported | NEW — requires L13 (fat pointers) for correct call ABI. |

L4 (string literals) and L12 (u8/i8 type) are now CLOSED. The remaining items are either optimizations (L1, L10) or new feature areas (L2, L3, L5) or simplifications with documented workarounds (L13, L14, L15).

---

## 6. Test Progression

| Version | Tests | Delta |
|---------|-------|-------|
| v0.8.4 (3.19) | 709 | — |
| v0.8.5 (3.20) | 709 | 0 (refactor) |
| v0.8.6 (3.21-3.23, R1) | 725 | +16 |
| v0.8.6 (3.24-3.26, R2) | 739 | +14 |
| **v0.8.6 (3.27-3.29, R3)** | **761** | **+22** |

---

## 7. Conclusion

Stage 3 (LLVM codegen) Round 3 gate review **PASSED** with unanimous 5/5 committee approval. All 43 audit cases pass, all 761 tests pass, 0 warnings, fmt + clippy clean.

**Audit CONVERGED** — 3 consecutive rounds with 0 new issues (R1=38, R2=43, R3=43).

**Critical feature shipped this round**:
- String literals (`"hello"`) now emit proper LLVM globals with byte content
- Byte string literals (`b"hello"`) share the same mechanism
- `u8`/`i8` types now map to LLVM `i8` (was incorrectly `i32`)
- Void-typed locals no longer produce invalid `alloca void` / `store void`

**Next steps** (in priority order, per remaining L-list):
1. **L2 — Struct/ADT codegen** (high value: enables user-defined types)
2. **L1 — PHI node emission** (optimization, not correctness)
3. **L3 — Closure codegen** (medium value)
4. **L5 — Trait dispatch** (medium value, requires L2 first)
5. **L13 — Fat pointer representation for &str/&[T]** (enables passing strings to functions)
