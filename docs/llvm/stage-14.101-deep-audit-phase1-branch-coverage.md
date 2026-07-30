# Stage 14.101 — Deep Audit Phase 1: Branch Coverage Analysis

> **Author**: redskaber
> **Date**: 2026-07-30
> **Stage**: 14.101
> **Version**: v0.114.0 → v0.115.0
> **Process**: stage-committee-process.md v3.22 §25 (D8 deep review)

## Overview

Stage 14.101 launched 3 parallel deep audits covering the entire compiler
pipeline. This document summarizes the data flow branch coverage findings
and the fixes applied in this stage.

## Audit Scope

| Pipeline | Files | LOC | Match stmts | Catch-all arms |
|----------|-------|-----|-------------|----------------|
| Frontend (lexer/parser/AST/HIR/resolve) | 41 | ~11,527 | 200 | 58 |
| Mid-end (MIR/typeck/borrowck/traits) | 35 | ~17,295 | 94 | 82 |
| Backend (codegen/LLVM/driver/stdlib/session) | 21 | ~13,000 | 130 | 58 |
| **Total** | **99** | **~42K** | **424** | **198** |

## P0 Bugs Identified (17 total)

### Frontend (5 P0)
1. `scan_expr_for_unresolved` catch-all — 6 HIR expr variants silently skipped
2. `scan_ty_for_unresolved` catch-all — 3 HIR ty variants silently skipped
3. `scan_pat_for_unresolved` — complete no-op stub
4. `lex_escape_from_str` silent fallback (`'\q'` → `'q'`)
5. `lex_hex`/`lex_oct``/`lex_bin` inconsistent suffix error reporting

### Mid-End (6 P0 + ~2,475 LOC dead code)
1. ME-1: `AggregateKind::Closure` → `Ty::Error` silently
2. ME-2: `Rvalue::BinaryOp` (Range) → `Ty::Error` silently
3. ME-3: Non-literal `Repeat` count → silently falls back to 1
4. ME-4: Const/static body lookup `_ => {}` silent
5. ME-5: Unknown macro → `Ty::Error` silently
6. ME-7: `place_ty` silent fallbacks for Deref/Index

**Dead code**: `mir/lvalue.rs` (250 LOC), `typeck/lifetime_elision.rs` (215 LOC),
`borrowck/drop_elaboration.rs` (282 LOC), `traits/object_safety.rs` (266 LOC),
`borrowck/region_inference.rs` (~1,100 LOC mostly dead).

### Backend (6 P0 + 5 performance hotspots)
1. SH-5: `LLVMSysEmitter::emit_checked_binop` stub
2. SH-7: `codegen_rvalue` catch-all returns "0"
3. SH-8: `Terminator::Drop` no-op
4. SH-2: `emit_dyn_trait_method_call` silent zero
5. SH-10: TextEmitter invalid IR for dyn Trait
6. SH-21: `scan_expr_for_unresolved` misses Try/Unsafe (same as frontend #1)

**Performance hotspots**: `LLVMSysEmitter::lookup()` ~37× per function, `cstr()`
leaks every CString, `LANDIN_DEBUG_CODEGEN` env var checked 8× via syscall,
driver 6 writeback passes per body, `resolve_self_param_type_for_sig` O(B×O×I).

## Stage 14.101 Fixes (3 families)

This stage fixes the 3 most critical families (all in `src/driver.rs`):

### Fix 1: scan_expr_for_unresolved — 6 missing arms

**Before** (catch-all silently skipped):
```rust
// Lit, Unit, Break, Continue, Try, Unsafe, MacroCall — no paths
_ => {}
```

**After** (explicit arms):
```rust
HirExprKind::Return { expr } | HirExprKind::Break { expr } => {
    if let Some(e) = expr { scan_expr_for_unresolved(e, errors); }
}
HirExprKind::Try { expr, .. } => scan_expr_for_unresolved(expr, errors),
HirExprKind::Unsafe(block) => { /* scan all stmts + trailing expr */ }
HirExprKind::MacroCall { path, .. } => {
    // Multi-segment only (built-in macros are single-segment)
    if path.segments.len() > 1 && matches!(path.res, Res::Unknown | Res::Err) {
        errors.resolve.push(/* cannot find macro */);
    }
}
HirExprKind::Await { expr, .. } => scan_expr_for_unresolved(expr, errors),
HirExprKind::Async { block, .. } => { /* scan all stmts + trailing expr */ }
HirExprKind::Lit(_) | HirExprKind::Unit | HirExprKind::Continue => {}
```

### Fix 2: scan_ty_for_unresolved — 3 missing arms

**Before** (catch-all):
```rust
_ => {}
```

**After**:
```rust
HirTyKind::FnPtr { inputs, output, .. } => {
    for t in inputs { scan_ty_for_unresolved(t, errors); }
    scan_ty_for_unresolved(output, errors);
}
HirTyKind::TraitObject { bounds, .. } => {
    for bound in bounds { scan_type_bound_for_unresolved(bound, errors); }
}
HirTyKind::ImplTrait(bounds) => {
    for bound in bounds { scan_type_bound_for_unresolved(bound, errors); }
}
// Added helper: scan_type_bound_for_unresolved
```

### Fix 3: scan_pat_for_unresolved — re-enabled

**Before** (no-op stub):
```rust
fn scan_pat_for_unresolved(_pat: &crate::hir::HirPat, _errors: &mut CompileErrors) {
    // G4 fix: temporarily disabled for patterns.
}
```

**After** (full implementation, 12 variants):
- `Wild`/`Rest`/`Lit` — no paths
- `Ident` — recurse into sub-pattern
- `Struct`/`TupleStruct`/`Path` — check multi-segment paths only
- `Tuple`/`Slice`/`Or` — recurse into sub-patterns
- `Range` — scan start/end expressions
- `Ref` — recurse into sub-pattern

## Verification

All 3 fixes verified with regression tests:
- `bk-0465-break-unresolved.lin` — `break nonexistent;` → resolve error ✅
- `bk-0466-unsafe-unresolved.lin` — `unsafe { let _ = nonexistent; }` → error ✅
- `bk-0467-fnptr-unresolved.lin` — `fn(unresolved_ty) -> i32` → error ✅
- `bk-0468-traitobject-unresolved.lin` — `dyn nonexistent_trait` → error ✅
- `bk-0469-pattern-unresolved.lin` — `nonexistent::Variant` in match → error ✅

## Remaining Work

12 P0 bugs remain (ME-1 to ME-7, SH-5/7/8/2/10) — deferred to Stage 14.102+.
~2,475 LOC dead code cleanup (P1). Performance optimization (P2). 48 hidden
problems for v0.2 catalogued (HP-1 to HP-23, HP-B1 to HP-B25).
