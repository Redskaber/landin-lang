# Stage 14.63 — Gate Review

> **Author**: redskaber
> **Date**: 2026-07-29
> **Version**: v0.78.0 → v0.79.0
> **Process**: stage-committee-process.md v3.22 §25 (D8 review)

## 1. Stage Summary

Stage 14.63 fixed three P0 bugs found through systematic audit of complex
patterns. All three were silent — compilation succeeded but runtime failed.
The bugs were discovered by writing test programs that exercised patterns
not covered by existing run_ok tests.

## 2. Bugs Fixed

### Bug 1: Mutual Recursion — Forward Declaration Deduplication

**Discovery**: Audit test `audit-stage14.63-b2.lin` failed with linker error
`undefined reference to 'landin_is_odd'` for mutually recursive functions
(`is_even`/`is_odd`).

**Root cause**: `LLVMSysEmitter::emit_function_begin` called `LLVMAddFunction`
without checking whether a forward declaration already existed. LLVM silently
renamed the new function (`foo` → `foo.1`), causing link errors.

**Fix**: Check `self.declared` cache + `LLVMGetNamedFunction` before calling
`LLVMAddFunction`. Reuse existing forward declaration if signature matches.

**Files changed**: `src/codegen/llvm/mod.rs` (1 function modified)

### Bug 2: Block-like Expression Statement Boundary

**Discovery**: Audit test `audit-stage14.63-b3-isolate.lin` failed with
typeck error `expected function, found Tuple([])` for code like:
```text
while cond { ... }
(n, acc)
```

**Root cause**: `Parser::parse_postfix_expr` greedily consumed `(` after any
expression as a Call. Block-like expressions at statement position are statement
boundaries — postfix `(` and `[` must NOT be consumed without explicit parens.

**Fix**: Added `is_block_like_expr(&Expr) -> bool` helper. In `parse_postfix_expr`,
after parsing primary, if `block_like`, the `LParen` and `LBracket` arms `break`
instead of consuming. `Dot` and `Question` are still allowed.

**Files changed**: `src/parser/expr.rs` (1 function modified + 1 helper added)

### Bug 3: Zero-field Struct Method Calls

**Discovery**: Audit test `audit-stage14.63-b5.lin` failed with LLVM module
verification error: `Call parameter type does not match function signature!
i32 0, ptr %v1 = call i32 @landin_value(i32 0)`.

**Root cause**: `mir_type_to_emit_type_with_layouts` mapped zero-field structs
to `EmitType::Void`. This caused:
1. `landin_new` had signature `void @landin_new()` (no return value)
2. The local `u` was skipped in the alloca loop (no storage)
3. `u.value()` had no alloca — passed `i32 0` as `&self`

**Fix**: Changed zero-field struct Adt case from `EmitType::Void` to
`EmitType::Struct(vec![])` (LLVM `{}` — empty struct, real value type).

**Files changed**: `src/codegen/mir_translation.rs` (1 case in match arm)

## 3. Audit Patterns Tested (No Bugs Found)

The following patterns were tested and all work correctly:

| Pattern | Example | Status |
|---------|---------|--------|
| Tuple struct field access | `Point(10, 20).0 + pt.1` | ✅ |
| Enum with tuple payload | `Opt::Some(42)` | ✅ |
| Recursive function | `fact(5) == 120` | ✅ |
| Nested struct via method | `o.get_val()` | ✅ |
| Method chaining | `Counter::new().inc().inc()` | ✅ |
| Multi-arm match with struct | `Shape::Rect(p1, p2)` | ✅ |
| Sequential `&mut self` | `c.inc(); c.inc(); c.inc()` | ✅ |
| Array of structs | `arr[0].r + arr[1].r` | ✅ |
| Nested tuple destructure | `((a, b), c) = ((1, 2), 3)` | ✅ |
| Nested match | `match c { Red => match n { 0 => 100, _ => 200 } }` | ✅ |
| While loop with mutation | `sum_to(5) == 10` | ✅ |
| Tuple returning function | `divmod(17, 5) = (3, 2)` | ✅ |
| Enum with struct payload | `Shape::Point(Point { x: 5, y: 10 })` | ✅ |
| Builder pattern (self-by-value) | `Builder::new().set(0, 10).set(1, 20)` | ✅ |
| Nested struct passed as `&mut` | `o.bump()` mutates `o.inner.val` | ✅ |
| Chained method calls on `&mut self` | `c.inc(); c.inc(); c.get()` | ✅ |
| Tuple returning tuple | `((1, 2), (3, 4))` | ✅ |
| Nested if-else chain | `grade(score)` 5-way branch | ✅ |
| Tuple struct with multiple fields | `Pair(10, 20)` | ✅ |
| Tuple struct swap method | `Pair(10, 20).swap() = Pair(20, 10)` | ✅ |
| Const folding | `2 + 3 * 4` == 14 | ✅ |
| Bool to int | `if t { 1 } else { 0 }` | ✅ |
| Nested struct field assignment | `c.b.a.x = c.b.a.x + 100` | ✅ |
| While with continue-style | `sum_odd_skip_even` | ✅ |
| Array element swap | `swap_array([1, 2, 3]) = [3, 2, 1]` | ✅ |
| Mutually recursive functions | `is_even(10) == true` | ✅ (Bug 1 fixed) |
| While + trailing tuple | `fact_pair(5) = (5, 120)` | ✅ (Bug 2 fixed) |
| Unit struct methods | `Unit::new().value() == 42` | ✅ (Bug 3 fixed) |
| Empty struct + impl | `struct Unit; impl Unit { ... }` | ✅ (Bug 3 fixed) |

## 4. Verification

- `cargo clean && cargo build --features llvm-backend` → ✅
- `cargo fmt` → ✅ (no changes)
- `cargo clippy --all-targets --features llvm-backend` → ✅ (0 warnings)
- `cargo test --features llvm-backend` → ✅ (1951 passed, 0 failed, 2 ignored)
- Conformance tests: 5134 (was 5131, +3 new run_ok)
- Pipeline coverage: 99.7% (671 paths, 669 verified)

## 5. Known Limitations (unchanged from v0.78.0)

| Limitation | Impact | GAP |
|------------|--------|-----|
| `for` loop not supported | Range iteration unavailable | v0.2 |
| `dyn Trait` runtime segfault | dyn dispatch crashes | GAP-30 |
| >4 bools in single println! | Wrong output | P2 |
| NLL too permissive | Unsound borrows accepted | GAP-1 |
| Region inference no-op | No lifetime constraints | GAP-2 |
| Drop elaboration no-op | No Drop::drop codegen | GAP-3 |
| Lifetime elision no-op | All lifetimes explicit | GAP-4 |
| Two-phase borrows missing | `vec.push(vec.len())` fails | GAP-6 |
| Closure-to-FnPtr coercion | Closures can't be passed as fn args | P1 (new) |
| No real stdlib | No Vec/String/Option | GAP-9 |
| Cross-module visibility stub | Private access allowed | GAP-14 |
| No mini-cargo CLI | No `landinc build` | GAP-15 |

## 6. D8 Review Dimensions

### D8.1 — Correctness
- All 3 fixes address real bugs (verified by isolated test cases)
- Zero regression in existing 1951 rust tests + 5131 conformance tests
- New tests cover the exact patterns that were broken

### D8.2 — Architecture
- Forward declaration deduplication is a localized fix in `emit_function_begin`
- Statement boundary check is a small addition to `parse_postfix_expr`
- ZST representation change is a single case in a match arm
- No architectural changes needed; existing structure accommodates the fixes

### D8.3 — API Naming
- New helper `is_block_like_expr` follows `<noun>_<noun>_<noun>` pattern per §23
- No public API changes (all fixes are internal)

### D8.4 — Design-Driven Testing
- 3 new run_ok tests, each directly tied to a specific bug:
  - E-106: mutual recursion (Bug 1)
  - E-107: while + tuple (Bug 2)
  - E-108: unit struct methods (Bug 3)

### D8.5 — Long-term vs Short-term
- Forward declaration deduplication: long-term fix (proper solution, not a workaround)
- Statement boundary: long-term fix (matches Rust grammar, no special cases)
- ZST as `Struct(vec![])`: long-term fix (correct semantic representation)

### D8.6 — Explicit vs Implicit
- Forward declaration reuse: explicit signature check before reuse (no silent mismatch)
- Statement boundary: explicit `block_like` flag (not magic token detection)
- ZST: explicit `Struct(vec![])` (not implicit `Void` elision)

### D8.7 — Errors vs Silent
- Forward declaration signature mismatch: falls back to LLVMAddFunction (renames) — surfaces issue
- Statement boundary: parser breaks loop instead of misparsing — surfaces issue at typeck
- ZST: LLVM module verification catches type mismatch — surfaces issue at codegen

### D8.8 — General vs Special-case
- Forward declaration: general mechanism (works for any function, not just recursive)
- Statement boundary: general rule (applies to all block-like expressions uniformly)
- ZST: general code path (same as non-empty structs, just with zero fields)

## 7. Stage Outcome

**Stage 14.63 PASSED** — three P0 bugs fixed, zero regression, 3 new run_ok tests.

**Next steps** (priority order):
1. Continue auditing complex patterns to find more silent bugs
2. Address closure-to-FnPtr coercion (newly identified P1)
3. Address remaining P0 blockers (GAP-4 lifetime elision, GAP-6 two-phase borrows)
4. Address deep soundness work (GAP-1 NLL, GAP-2 region inference, GAP-3 drop elaboration)
