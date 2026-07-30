# Stage 14.101 — Gate Review: Deep Architecture Audit Phase 1

> **Author**: redskaber
> **Date**: 2026-07-30
> **Version**: v0.114.0 → v0.115.0
> **Process**: stage-committee-process.md v3.22 §25 (D8 deep review)

## 1. Stage Summary

Stage 14.101 is the first stage of a deep architecture audit requested by the
user. The user asked for:
1. Data flow branch coverage audit (all enums/branches fully covered, no silent handling)
2. Architecture design audit per pipeline stage
3. Next-stage design + implementation + test triple coverage
4. Hidden problem analysis (will entering next stage make problems more complex?)
5. Performance baseline measurement
6. Multi-round deep verification before declaring v0.1 truly ready

This stage launched 3 parallel general-purpose subagents to audit:
- Frontend pipeline (lexer/parser/AST/HIR/resolve) — 41 files, ~11,527 LOC
- Mid-end pipeline (MIR/typeck/borrowck/traits) — 35 files, ~17,295 LOC
- Backend pipeline (codegen/LLVM/driver/stdlib/session) — 21 files, ~13K LOC

## 2. Audit Findings Summary

### Frontend Audit (P0 bugs found: 5)

1. **scan_expr_for_unresolved catch-all** — silently skips 6 HIR expr variants:
   `Break { expr }`, `Try { expr }`, `Unsafe(block)`, `MacroCall { path }`,
   `Async { block }`, `Await { expr }`. Examples: `break nonexistent;` silently
   compiles.

2. **scan_ty_for_unresolved catch-all** — silently skips `FnPtr`, `TraitObject`,
   `ImplTrait`. Examples: `fn(unresolved) -> i32` goes unreported.

3. **scan_pat_for_unresolved is a complete no-op stub** — patterns never scanned.

4. **lex_escape_from_str silent fallback** — `'\q'` silently becomes `'q'`.

5. **lex_hex/lex_oct/lex_bin inconsistent suffix error reporting**.

### Mid-End Audit (P0 bugs found: 6, dead code: ~2,475 LOC)

1. **ME-1**: `AggregateKind::Closure` → `Ty::Error` silently (checker.rs:877)
2. **ME-2**: `Rvalue::BinaryOp` (Range) → `Ty::Error` silently (checker.rs:779)
3. **ME-3**: Non-literal `Repeat` count → silently falls back to 1 element
4. **ME-4**: Const/static body lookup `_ => {}` silent
5. **ME-5**: Unknown macro → `Ty::Error` silently
6. **ME-7**: `place_ty` silent fallbacks for Deref/Index on wrong types

**Dead code**: `mir/lvalue.rs` (250 LOC, orphaned), `typeck/lifetime_elision.rs`
(215 LOC), `borrowck/drop_elaboration.rs` (282 LOC), `traits/object_safety.rs`
(266 LOC), `borrowck/region_inference.rs` (~1,100 LOC mostly dead).

### Backend Audit (P0 bugs found: 6, performance hotspots: 5)

1. **SH-5**: `LLVMSysEmitter::emit_checked_binop` stub — silently disables
   overflow detection on `--emit-obj`/`--run` path
2. **SH-10**: TextEmitter produces invalid LLVM IR for `dyn Trait` calls
3. **SH-7**: `codegen_rvalue` catch-all returns `"0"` — swallows
   `Discriminant`/`Len`/`Repeat`
4. **SH-8**: `Terminator::Drop` is a complete no-op — `impl Drop` never called
5. **SH-2**: `emit_dyn_trait_method_call` returns silent zero on missing vtable
6. **SH-21**: `scan_expr_for_unresolved` misses `Try`/`Unsafe` (same as frontend)

**Performance hotspots**: `LLVMSysEmitter::lookup()` ~37× per function,
`cstr()` leaks every CString, `LANDIN_DEBUG_CODEGEN` env var checked 8× via
syscall, driver 6 writeback passes per body, `resolve_self_param_type_for_sig`
is O(B × O × I) quadratic.

## 3. Stage 14.101 Fixes (P0 silent-handling bugs)

This stage fixes the most critical P0 bugs from the frontend audit (the
`scan_expr_for_unresolved` / `scan_ty_for_unresolved` / `scan_pat_for_unresolved`
family). These were chosen first because:
- They share the same root cause (catch-all `_ => {}` arms)
- They're the same anti-pattern that caused Stage 14.100 AA1-AA4 bugs
- They're localized to `src/driver.rs` (one file)
- They directly violate §1.0 原则 5 "报错 > 静默"

### Fix 1: scan_expr_for_unresolved — added 6 missing arms

**Before**: `_ => {}` catch-all silently skipped `Break`, `Try`, `Unsafe`,
`MacroCall`, `Await`, `Async`.

**After**: Explicit arms for all 6 variants:
- `Break { expr }` / `Return { expr }` — scan expr if present
- `Try { expr }` — scan expr
- `Unsafe(block)` — scan all stmts + trailing expr
- `MacroCall { path }` — check path (multi-segment only; built-in macros
  like `vec!` are single-segment and handled specially)
- `Await { expr }` — scan expr
- `Async { block }` — scan all stmts + trailing expr

### Fix 2: scan_ty_for_unresolved — added 3 missing arms

**Before**: `_ => {}` catch-all silently skipped `FnPtr`, `TraitObject`,
`ImplTrait`.

**After**: Explicit arms:
- `FnPtr { inputs, output, .. }` — scan all inputs + output
- `TraitObject { bounds, .. }` — scan all bounds via `scan_type_bound_for_unresolved`
- `ImplTrait(bounds)` — scan all bounds

### Fix 3: scan_pat_for_unresolved — re-enabled (was no-op stub)

**Before**: Function body was `// G4 fix: temporarily disabled for patterns.`

**After**: Full implementation handling all 12 `HirPatKind` variants:
- `Wild`, `Rest`, `Lit` — no paths
- `Ident` — binds new variable, recurse into sub-pattern
- `Struct`, `TupleStruct`, `Path` — check multi-segment paths only
  (single-segment might be lazily-resolved enum variants)
- `Tuple`, `Slice`, `Or` — recurse into sub-patterns
- `Range` — scan start/end expressions
- `Ref` — recurse into sub-pattern

## 4. Test Count Updates

| Suite | Before | After | Delta |
|-------|--------|-------|-------|
| Rust tests | 1951 | 1951 | 0 |
| Conformance tests | 5204 | 5209 | +5 |

New tests:
- `bk-0465-break-unresolved.lin` — Break expr unresolved (compile_error)
- `bk-0466-unsafe-unresolved.lin` — Unsafe block unresolved (compile_error)
- `bk-0467-fnptr-unresolved.lin` — FnPtr type unresolved (compile_error)
- `bk-0468-traitobject-unresolved.lin` — TraitObject unresolved (compile_error)
- `bk-0469-pattern-unresolved.lin` — Pattern multi-segment path unresolved (compile_error)

## 5. Verification

```
cargo build --release --features llvm-backend: ✅
cargo fmt: ✅ clean
cargo clippy --all-targets --features llvm-backend: ✅ 0 warnings
cargo test --features llvm-backend: ✅ 1951 passed, 0 failed
python3 tests/conformance/run_all.py: ✅ 5209 passed, 0 failed
```

## 6. Remaining P0 Bugs (deferred to Stage 14.102+)

The following P0 bugs were identified by the deep audit but not yet fixed:

### From Frontend Audit:
- `lex_escape_from_str` silent fallback (`'\q'` → `'q'`)
- `lex_hex`/`lex_oct`/`lex_bin` inconsistent suffix error reporting

### From Mid-End Audit:
- ME-1: `AggregateKind::Closure` → `Ty::Error` silently
- ME-2: `Rvalue::BinaryOp` (Range) → `Ty::Error` silently
- ME-3: Non-literal `Repeat` count → silently falls back to 1
- ME-4: Const/static body lookup silent
- ME-5: Unknown macro → `Ty::Error` silently
- ME-7: `place_ty` silent fallbacks

### From Backend Audit:
- SH-5: `LLVMSysEmitter::emit_checked_binop` stub
- SH-7: `codegen_rvalue` catch-all returns "0"
- SH-8: `Terminator::Drop` no-op
- SH-2: `emit_dyn_trait_method_call` silent zero
- SH-10: TextEmitter invalid IR for dyn Trait

### Dead Code (P1 cleanup):
- `mir/lvalue.rs` (250 LOC, orphaned)
- `typeck/lifetime_elision.rs` (215 LOC, dead)
- `borrowck/drop_elaboration.rs` (282 LOC, dead)
- `traits/object_safety.rs` (266 LOC, dead)
- `borrowck/region_inference.rs` (~1,100 LOC mostly dead)

## 7. Stage Verdict

**PASS** — Fixed 3 families of P0 silent-handling bugs (scan_expr/ty/pat).
+5 new regression tests. No regressions.

Per §1.0 原则 5 "报错 > 静默": 5 new error cases now produce clear resolve
errors instead of silent wrong output.

Per §1.0 原则 6 "通用 > 特例": One unified scan function per kind (expr/ty/pat)
handles all variants — no per-variant special-casing outside the match arms.

v0.115.0: minor bump (3 families of P0 fixes — important correctness improvements)
