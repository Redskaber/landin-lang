# Stage 14.65 — Gate Review

> **Author**: redskaber
> **Date**: 2026-07-29
> **Version**: v0.80.0 → v0.81.0
> **Process**: stage-committee-process.md v3.22 §25 (D8 review)

## 1. Stage Summary

Stage 14.65 fixed four more P0 bugs found through systematic audit of complex
patterns involving floats, chars, bools, and function pointers. All four were
silent — compilation succeeded but runtime produced wrong values or segfaulted.

## 2. Bugs Fixed

### Bug 1: Integer-to-integer casts used BitCast

**Discovery**: Audit test `audit-stage14.65-char-cast.lin` failed with
`Invalid bitcast` LLVM verification errors for `c as i32` and `n as char`.

**Root cause**: `emit_cast` only handled specific pairs (I32↔I64, I1↔I32).
Other integer pairs fell through to `LLVMBuildBitCast`, invalid for different
widths.

**Fix**: Use `LLVMBuildIntCast2` for ANY integer-to-integer cast. Handles
zext/sext/trunc automatically.

**Files changed**: `src/codegen/llvm/mod.rs` (emit_cast rewritten),
`src/codegen/text/mod.rs` (emit_cast rewritten for consistency)

### Bug 2: Comparison results stored with operand type

**Discovery**: Audit test `audit-stage14.65.lin` segfaulted at runtime for
`is_positive(5.0)` — a float comparison returning bool.

**Root cause**: typeck writeback propagated operand types to BinaryOp results.
For `x > 0.0` (f64), it overwrote the result type with f64, causing
`store double %cmp_result, %bool_alloca`.

**Fix**: Skip operand-type propagation for comparison ops (Eq/Ne/Lt/Le/Gt/Ge).
These always return Bool.

**Files changed**: `src/typeck/checker.rs` (writeback second pass updated)

### Bug 3: Bool match with both true and false arms

**Discovery**: Audit test `audit-stage14.65-bool-match.lin` returned
`-976284176` (garbage) for `bool_to_str(false)` instead of `0`.

**Root cause**: `SwitchInt` codegen assumed "false goes to otherwise" and
only checked for the `true` target, ignoring the `false` arm's body.

**Fix**: Check for BOTH `true` and `false` targets. Branch to each if present.

**Files changed**: `src/codegen/terminator.rs` (SwitchInt bool case updated)

### Bug 4: Function pointer return with forward reference

**Discovery**: Audit test `audit-stage14.65.lin` segfaulted after `safe_mul`
output (200) — the `adder(5)` call returned a null function pointer.

**Root cause**: `interpret_adhoc` called `LLVMGetNamedFunction` which returned
null for not-yet-emitted functions. The code returned `LLVMConstNull` (null
pointer), which was stored and later called.

**Fix**: Added `fn_sigs` field to `LLVMSysEmitter`. `interpret_adhoc` looks
up the function's signature and creates a forward declaration with the CORRECT
signature. `emit_function_begin` reuses this declaration (Stage 14.63 dedup).

**Files changed**: `src/codegen/llvm/mod.rs` (fn_sigs field + set_fn_sigs
method + interpret_adhoc updated), `src/codegen/mod.rs` (build_fn_sigs_map +
set_fn_sigs call)

## 3. Audit Patterns Tested (No Bugs Found)

The following patterns were tested and all work correctly:

| Pattern | Example | Status |
|---------|---------|--------|
| Float arithmetic | `circle_area(2.0)` = 12.566360 | ✅ |
| Float comparison to Bool | `is_positive(5.0)` = true | ✅ (Bug 2 fixed) |
| Char cast to int and back | `next_char('a')` = 'b' (98) | ✅ (Bug 1 fixed) |
| Float struct with methods | `Point3d::new(1,2,3).dot(&p2)` = 32.0 | ✅ |
| i32 overflow check | `safe_mul(10, 20)` = 200 | ✅ |
| Function returning fn pointer | `adder(5)(21)` = 42 | ✅ (Bug 4 fixed) |
| Array of floats | `sum_floats([1.5, 2.5, 3.0, 4.0])` = 11.0 | ✅ |
| Array of bools | `count_true([T, F, T, T, F])` = 3 | ✅ |
| Tuple with mixed types | `make_pair()` = (42, 3.14, true) | ✅ |
| Bool match with both arms | `bool_to_str(true)` = 1, `bool_to_str(false)` = 0 | ✅ (Bug 3 fixed) |

## 4. Verification

- `cargo clean && cargo build --features llvm-backend` → ✅
- `cargo fmt` → ✅ (no changes)
- `cargo clippy --all-targets --features llvm-backend` → ✅ (0 warnings)
- `cargo test --features llvm-backend` → ✅ (1951 passed, 0 failed, 2 ignored)
- Conformance tests: 5141 (was 5137, +4 new run_ok)
- Pipeline coverage: 99.7% (678 paths, 676 verified)

## 5. D8 Review Dimensions

### D8.1 — Correctness
- All 4 fixes address real bugs (verified by isolated test cases)
- Zero regression in existing 1951 rust tests + 5137 conformance tests
- New tests cover the exact patterns that were broken

### D8.2 — Architecture
- Integer cast generalization is a single rule (IntCast2) replacing enumeration
- Comparison writeback fix is a targeted skip in the writeback pass
- Bool match fix is a small addition to the SwitchInt codegen
- Fn pointer forward ref adds a `fn_sigs` field (clean separation of concerns)

### D8.3 — API Naming
- `set_fn_sigs`, `build_fn_sigs_map`, `predeclare_function` follow
  `<verb>_<noun>` pattern per §23
- No public API changes (all fixes are internal)

### D8.4 — Design-Driven Testing
- 4 new run_ok tests, each directly tied to a specific bug:
  - E-112: char cast (Bug 1)
  - E-113: float comparison (Bug 2)
  - E-114: bool match (Bug 3)
  - E-115: fn pointer return (Bug 4)

### D8.5 — Long-term vs Short-term
- Integer cast: long-term (general mechanism, handles all widths)
- Comparison writeback: long-term (correct semantic — comparisons return Bool)
- Bool match: long-term (correct semantic — both arms execute)
- Fn pointer forward ref: long-term (fn_sigs map is the right abstraction)

### D8.6 — Explicit vs Implicit
- Integer cast: explicit type kind check before choosing IntCast2
- Comparison writeback: explicit op check (is_comparison) before skip
- Bool match: explicit check for both true AND false targets
- Fn pointer forward ref: explicit fn_sigs lookup before creating forward decl

### D8.7 — Errors vs Silent
- All four bugs were silent (wrong values or segfaults, no compile error)
- Integer cast: BitCast errors now surface as IntCast2 (correct op)
- Comparison writeback: type mismatch now avoided (Bool stays Bool)
- Bool match: false arm now executes (no silent skip)
- Fn pointer forward ref: null pointer now replaced with real forward decl

### D8.8 — General vs Special-case
- Integer cast: general rule for ALL integer pairs
- Comparison writeback: general skip for ALL comparison ops
- Bool match: general check for both true AND false (not just true)
- Fn pointer forward ref: general fn_sigs map (handles any forward reference)

## 6. Stage Outcome

**Stage 14.65 PASSED** — four more P0 bugs fixed, zero regression, 4 new
run_ok tests.

**Next steps** (priority order):
1. Continue auditing complex patterns (generics, trait dispatch, closures)
2. Address closure-to-FnPtr coercion (P1, identified in Stage 14.63)
3. Address remaining P0 blockers (GAP-4 lifetime elision, GAP-6 two-phase borrows)
4. Address deep soundness work (GAP-1 NLL, GAP-2 region inference, GAP-3 drop elaboration)
