# Stage 18.252 — Task Review: TD-SPAN-DUMMY-CLEANUP + TD-STDLIB-FACADE Audit

> **Date**: 2026-08-24
> **Version**: v0.492.0 (no bump — audit + documentation)
> **Task ID**: stage18.252
> **Reviewer**: Super Z (main) — ARCH-A + REV-A + QA-A

## 1. 触发场景

Per tech-debt-register: TD-SPAN-DUMMY-CLEANUP (🟡 Partial) and
TD-STDLIB-FACADE (🟡 Split) are stale entries that need status update.

## 2. Audit Results

### 2.1 TD-SPAN-DUMMY-CLEANUP

**Current state**: Stage 18.159 fixed 2 discriminant spans in expr_variants.rs.
Remaining Span::DUMMY instances in production code:

| File | Usage | Assessment |
|------|-------|------------|
| typeck/check.rs:202-205 | `if stmt.span != Span::DUMMY { ... } else { Span::DUMMY }` | ✅ Legitimate — fallback when stmt has no span |
| typeck/check.rs:230-401 | `if stmt.span != Span::DUMMY` / `if term.span != Span::DUMMY` | ✅ Legitimate — checking for DUMMY span before using it |
| expr_variants.rs:245,248 | `Ty::new(TyKind::Error, Span::DUMMY)` | ✅ Legitimate — Error type has no source span (synthesized) |
| expr_variants.rs:273 | `Place::local(*l, Span::DUMMY)` | ✅ Legitimate — synthesized MIR places (no source location) |
| expr_variants.rs:540 | `fresh_infer_ty(Span::DUMMY)` | ✅ Legitimate — fresh inference variable (no source location) |

**Decision**: ✅ Resolved — all remaining Span::DUMMY instances are legitimate
synthesized values with no source span (per §1.0 原則 3 显式 > 隐式).
No further action needed.

### 2.2 TD-STDLIB-FACADE

**Original description**: "String/Vec/Option/Result are type stubs, not real implementations"

**Current state**: These are ALL real implementations now:
- `Option<T>`: Real enum with `is_some`/`is_none`/`unwrap_or` methods (prelude)
- `Result<T, E>`: Real enum with `is_ok`/`is_err`/`unwrap_or` methods (prelude)
- `String`: Real struct `{ ptr, len, cap }` with `new`/`len`/`from_str`/`push_str`/`as_str` (prelude + MIR intrinsics)
- `Vec<T>`: Real struct `{ ptr, len, cap }` with `new`/`len`/`push`/`get` (prelude + MIR intrinsics)
- `Box<T>`: Real struct `(*mut T)` with `new` intrinsic + auto-drop (prelude + MIR intrinsic + drop glue)

All have:
- Real heap allocation (`__landin_alloc`/`__landin_dealloc`/`__landin_realloc`)
- Real methods (prelude impl + MIR intrinsic lowering)
- Real type checking (typeck validates method calls)
- Real borrow checking (borrowck validates `&mut self`)

**Decision**: ✅ Resolved — String/Vec/Option/Result/Box are all real implementations
with heap allocation, methods, and full compiler support. No longer stubs.

## 3. Conclusion

Both TDs are stale and should be closed:
- TD-SPAN-DUMMY-CLEANUP: ✅ Resolved (all remaining DUMMY are legitimate)
- TD-STDLIB-FACADE: ✅ Resolved (all types are real implementations)

No code changes needed.
