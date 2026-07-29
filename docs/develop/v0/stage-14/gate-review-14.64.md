# Stage 14.64 — Gate Review

> **Author**: redskaber
> **Date**: 2026-07-29
> **Version**: v0.79.0 → v0.80.0
> **Process**: stage-committee-process.md v3.22 §25 (D8 review)

## 1. Stage Summary

Stage 14.64 fixed three more P0 bugs found through systematic audit of complex
patterns. All three were silent — compilation succeeded but runtime produced
wrong values. The bugs were discovered by writing test programs that exercised
patterns not covered by existing run_ok tests.

## 2. Bugs Fixed

### Bug 1: Comparison Results Stored to Bool Locals

**Discovery**: Audit test `audit-stage14.64-bubble.lin` produced
`0 0 1 2 4` instead of `3 1 4 2 5` for `bubble_sort_pass([5, 3, 1, 4, 2])`.

**Root cause**: Comparison ops in `codegen_rvalue` always zext i1 results to
i32. Storing i32 to a Bool (i1) alloca caused a type mismatch that the
LLVMSysEmitter silently ignored (its `emit_store` discarded the type parameter).

**Fix**: In `codegen_statement`, when storing to an i1 local AND the rvalue
is a comparison, trunc the i32 value to i1 via `emit_cast(I32, I1, val)`.

**Files changed**: `src/codegen/statement.rs` (1 conditional added)

### Bug 2: i64 Constants Stored as i32

**Discovery**: Audit test `audit-stage14.64.lin` produced
`180228417674752` instead of `3000000000` for
`big_sum(1_000_000_000, 2_000_000_000)` (only when combined with other functions).

**Root cause**: `LLVMSysEmitter::emit_const` always creates i32 constants for
`ConstVal::Int`. Storing i32 to an i64 alloca only writes 4 bytes, leaving
upper 4 bytes as garbage.

**Fix** (two parts):
1. `src/codegen/operand.rs`: Cast integer constants to their declared type
   (`c.ty`) after `emit_const` — only for integer types.
2. `src/codegen/llvm/mod.rs`: `emit_store` now checks the value's LLVM type
   and casts integer values to match the alloca's type via `LLVMBuildIntCast2`.

**Files changed**: `src/codegen/operand.rs` (1 cast added),
`src/codegen/llvm/mod.rs` (emit_store rewritten)

### Bug 3: Field Index Resolution for Ambiguous Names

**Discovery**: Audit test `audit-stage14.64.lin` produced `1 1` instead of
`1 0` for `unit_x().y` (only when another struct `Point2` with same field
names was in the compilation unit).

**Root cause**: `resolve_field_index`'s fallback search marked the search as
"ambiguous" when multiple structs had the same field name, and fell through
to `return 0` — even when all structs agreed on the field index.

**Fix**: Track whether ALL found indices agree. If they agree, return the
index. Only fall through to `return 0` if the indices truly disagree.

**Files changed**: `src/mir/lower/field_resolution.rs` (fallback rewritten)

## 3. Audit Patterns Tested (No Bugs Found)

The following patterns were tested and all work correctly:

| Pattern | Example | Status |
|---------|---------|--------|
| Negative number args | `double_neg(42)` = 42 | ✅ |
| i64 arithmetic | `big_sum(1B, 2B)` = 3B | ✅ (Bug 2 fixed) |
| Struct method returning &str | `p.greet()` = 30 | ✅ |
| Enum tuple + match | `eval(Expr2::Add(10, 20))` = 30 | ✅ |
| Function returning struct | `origin()` = Vec2{0,0} | ✅ |
| Collatz sequence | `collatz_steps(27)` = 111 | ✅ |
| Bubble sort one pass | `bubble_sort_pass([5,3,1,4,2])` = [3,1,4,2,5] | ✅ (Bug 1 fixed) |
| Nested match + struct destructure | `handle_e(E::A(Point2{10,20}))` = 30 | ✅ |
| Multi-struct field access | `unit_x().y` = 0 (with Point2 present) | ✅ (Bug 3 fixed) |

## 4. Verification

- `cargo clean && cargo build --features llvm-backend` → ✅
- `cargo fmt` → ✅ (no changes)
- `cargo clippy --all-targets --features llvm-backend` → ✅ (0 warnings)
- `cargo test --features llvm-backend` → ✅ (1951 passed, 0 failed, 2 ignored)
- Conformance tests: 5137 (was 5134, +3 new run_ok)
- Pipeline coverage: 99.7% (674 paths, 672 verified)

## 5. D8 Review Dimensions

### D8.1 — Correctness
- All 3 fixes address real bugs (verified by isolated test cases)
- Zero regression in existing 1951 rust tests + 5134 conformance tests
- New tests cover the exact patterns that were broken

### D8.2 — Architecture
- Bool truncation is a targeted fix in `codegen_statement`
- Constant cast is in `codegen_operand` (single point for all constants)
- Store-level coercion is defense-in-depth in `emit_store`
- Three-layer fix ensures robustness against future similar bugs

### D8.3 — API Naming
- No public API changes (all fixes are internal)
- New helper logic uses existing `emit_cast` API

### D8.4 — Design-Driven Testing
- 3 new run_ok tests, each directly tied to a specific bug:
  - E-109: bubble sort (Bug 1)
  - E-110: i64 arithmetic (Bug 2)
  - E-111: multi-struct field access (Bug 3)

### D8.5 — Long-term vs Short-term
- All three fixes are long-term (proper type coercion, not workarounds)
- The store-level coercion (Bug 2 fix part 2) is a general mechanism that
  will catch future type mismatches automatically

### D8.6 — Explicit vs Implicit
- Bool trunc: explicit check for comparison rvalue type
- Constant cast: explicit check for integer type pair compatibility
- Store coercion: explicit type check via `LLVMTypeOf`

### D8.7 — Errors vs Silent
- All three bugs were silent (wrong runtime values, no compile error)
- The fixes surface the type mismatches as explicit cast instructions
- Non-integer type mismatches in `emit_store` now surface as LLVM
  verification errors (instead of silent bitcasts)

### D8.8 — General vs Special-case
- Bool trunc: specific to comparison→Bool case (acceptable — the general
  fix is the store-level coercion)
- Constant cast: general for all integer constants
- Store coercion: general for all integer type mismatches

## 6. Stage Outcome

**Stage 14.64 PASSED** — three more P0 bugs fixed, zero regression, 3 new
run_ok tests.

**Next steps** (priority order):
1. Continue auditing complex patterns to find more silent bugs
2. Address closure-to-FnPtr coercion (P1, identified in Stage 14.63)
3. Address remaining P0 blockers (GAP-4 lifetime elision, GAP-6 two-phase borrows)
4. Address deep soundness work (GAP-1 NLL, GAP-2 region inference, GAP-3 drop elaboration)
