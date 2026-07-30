# Stage 14.98 — Gate Review

> **Author**: redskaber
> **Date**: 2026-07-30
> **Version**: v0.111.0 → v0.112.0
> **Process**: stage-committee-process.md v3.22 §25 (D8 review)

## 1. Stage Summary

Stage 14.98 fixes 4 P0 bugs found by the Round 7 independent audit. All 4 are
LLVM crashes on common code patterns (method calls on let-bound locals whose
init type isn't propagated by typeck). All are fully fixed.

## 2. Bugs Fixed

### Bug Z1/Z2: Method call on struct literal inside loop/match crashes

**Symptom**: `for i in 0..3 { let n = N { v: i }; sum += n.base(); }` crashed
with `LLVM module verification failed: Called function must be a pointer!`.

Same crash for `let n = match x { 0 => N{v:100}, _ => N{v:200} }; n.base();`.

**Root cause**: `search_expr_for_local_init` (and the underlying
`search_block_for_local`) only handled `Block` and `If` — they didn't recurse
into `While`/`For`/`Loop`/`Match` bodies. When typeck didn't propagate the
local's type, method resolution failed and emitted `Const{ty:Error, val:Int(0)}`
→ null function pointer → LLVM crash.

**Fix** (`src/mir/lower/expr_operand.rs`):
1. Rewrote `search_expr_for_local_init` to handle all expression kinds:
   `Block`, `If`, `While`, `For`, `Loop`, `Match` (with arm guard + body search).
2. Added `search_expr_for_local_init_expr` helper that returns the init
   expression (not yet type-resolved) — used by `find_local_init_expr`.
3. Removed old `search_block_for_local_init_expr` (only handled Block).
4. For `Match` init: look at the first arm's body to determine the type
   (all arms have the same type per typeck).
5. `find_local_init_type` now also handles `Call` init (free function calls)
   and `MethodCall` init (chained method calls) by querying the function's
   return type via `query_method_return_type`.

### Bug Z3: Trait default body via intermediate `let` crashes

**Symptom**: `let r1 = p; let r2 = r1.g();` where `g` is a trait default body
crashed with LLVM null function pointer.

**Root cause**: `resolve_inherent_method_from_hir_expr`'s MethodCall-init
tracing arm only called `resolve_inherent_method`, not `resolve_trait_method`.
When the traced method was a trait default body, the resolution failed.

**Fix**: Added `.or_else(|| resolve_trait_method(hir, &ret_ty, method_name))`
to all 3 method-resolution arms in `resolve_inherent_method_from_hir_expr`:
- MethodCall-init tracing arm
- Static method call (Call with Fn DefKind) arm
- MethodCall receiver arm

### Bug Z4: Method call on free function result crashes

**Symptom**: `let n = make_n(i); n.base();` (where `make_n` is a free function
returning a struct) crashed with LLVM null function pointer.

**Root cause**: `query_method_return_type` only searched `Impl` blocks, not
free `Fn` owners. Free-function return types couldn't be traced.

**Fix**: Extended `query_method_return_type` to search:
1. `HirItem::Impl` blocks (existing)
2. `HirItem::Fn` top-level free functions (new)
3. `HirItem::Trait` trait default body methods (new — for `Self` return types,
   uses the first impl's self_ty as the specialization type)

## 3. Test Count Updates

| Suite | Before | After | Delta |
|-------|--------|-------|-------|
| Rust tests | 1951 | 1951 | 0 |
| Conformance tests | 5191 | 5195 | +4 |

New run_ok tests:
- `e2e-runok-163-method-on-struct-in-loop.lin` — Bug Z1 (struct in for-loop)
- `e2e-runok-164-method-on-struct-from-match.lin` — Bug Z2 (struct from match)
- `e2e-runok-165-method-on-free-fn-result-in-loop.lin` — Bug Z4 (free fn result)
- `e2e-runok-166-trait-default-via-let.lin` — Bug Z3 (trait default via let)

## 4. Verification

```
cargo build --release --features llvm-backend: ✅
cargo fmt: ✅ clean
cargo clippy --all-targets --features llvm-backend: ✅ 0 warnings
cargo test --features llvm-backend: ✅ 1951 passed, 0 failed
python3 tests/conformance/run_all.py: ✅ 5195 passed, 0 failed
```

## 5. Known Limitations (carried forward)

- For-loop over arrays: not supported (clear error message)
- Open ranges (`..end`, `start..`): not supported (clear error message)
- Trait default body with multiple impls: uses first impl's self_ty
  (v0.1 single-impl heuristic — full monomorphization is v0.2+ work)
- Trait default body calling another trait's method: not supported
- For-loop mutability: loop variable is mutable even without `mut` (Bug Z5/P1)
- For-loop var modification affects iteration (Bug Z6/P1) — modifying `i`
  inside the body changes the counter

## 6. Stage Verdict

**PASS** — All 4 P0 bugs found by Round 7 audit are fully fixed. No regressions.
+4 new run_ok regression tests.

Per §1.0 原则 5 "报错 > 静默": All 4 bugs were LLVM crashes (loud failures).
Now they work correctly.

Per §1.0 原则 6 "通用 > 特例": One unified `search_expr_for_local_init_expr`
handles all expression kinds (Block, If, While, For, Loop, Match) — no
per-kind special-casing.

Per §1.0 原则 1 "长期 > 短期": The fix is at the right architectural layer
(HIR traversal for type tracing), not a hack at codegen.

v0.112.0: minor bump (4 P0 fixes — important correctness improvements)
