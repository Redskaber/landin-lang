# Landin Compiler — Release Notes

**Author**: redskaber
**Current version**: v0.361.0
**Date**: 2026-08-09
**Test count**: 638 rust lib tests + 2648 integration tests + 2935 conformance tests + 7 fuzz tests = 6228 total (100% pass rate)

---
## v0.361.0 — Stage 18.93 (Deep Audit v4 + Final Polish)

### Overview

Final audit pass confirming v0.1 stable release readiness. Pipeline is
**audit-clean** — all Stage 18.71-18.92 fixes verified, 0 remaining issues.

### Audit Results

| Dimension | Status |
|-----------|--------|
| Error system (8 Kind enums + E001-E900) | ✅ Clean |
| Production panic/unwrap | ✅ Clean (0 panic, all unwrap guarded) |
| Span::DUMMY in error reporting | ✅ Clean (unify span param) |
| API naming | ✅ Clean (85+ renames) |
| Dead code | ✅ Clean (documented) |
| Debug format leaks | ✅ Clean |
| Incremental compilation | ✅ Removed (no remnants) |

### Polish Fixes

1. `bin/main.rs`: `to_str().unwrap()` → `to_string_lossy()` (non-UTF8 path safety)
2. `driver.rs`: missing-main `Span::DUMMY` → `Span::new(0, src.len())`
3. `typeck/checker.rs`: simplified redundant span conditional
4. `codegen/llvm/mod.rs`: fixed cache key comment

### v0.1 Stable Release Summary

Stage 18.71-18.93 completed the full audit fix cycle:
- 13 P0/P1 typeck validation fixes (121 tests flipped)
- 3 deep audits (v1/v2/v3/v4)
- Error system fully structured (8 Kind enums + E001-E900)
- Test system enhanced (fuzz + diagnostic quality + dedup 5348→2935)
- Cross-compilation complete (Phase 1-3: x86_64 + AArch64)
- GATs Phase 1-3 complete
- API naming standardized (85+ renames)
- Span::DUMMY cleaned (unify span parameter)

---
## v0.360.0 — Stage 18.92 (Error Type Kind Enums)

Added Kind enums to all 5 remaining error types (LexError/ParseError/LowerError/CodegenError/MacroError). All 8 error types now have structured Kind enums.

---
## v0.358.0 — Stage 18.90 (Cross-Compilation Phase 3)

Fixed `to_object_file` to use configured target triple instead of host triple. Cross-compilation to AArch64 verified.

---
## v0.356.0 — Stage 18.88 (Cross-Compilation Foundation)

Added `TargetTriple` type + `with_target()` constructors. Removed hardcoded target triple from both emitters.

---
## v0.355.0 — Stage 18.87 (GATs Phase 3)

Fixed projection resolver bugs B6/B7/B8: added FnDef/FnPtr/Closure recursive resolution, expanded types_match to 20+ variants, added recursion depth limit.

---
## v0.353.0 — Stage 18.85 (Systematic Test Enhancement)

Added 7 fuzz/stress tests: random programs, malformed input, large match/struct/array, deep nesting, many functions.

---
## v0.354.0 — Stage 18.86 (Diagnostic Quality)

Replaced 115/157 generic `ERROR_PATTERN: error` with specific patterns (73% replacement rate).

---
## v0.346.0 — Stage 18.78 (P0 Correctness Patch)

Wired `CompileErrors.lower` and `CompileErrors.codegen` fields. HIR lowering errors and codegen errors now properly collected and displayed.

---
## v0.343.0 — Stage 18.75 (P0 Error System Fixes)

Added `lower` + `codegen` fields to CompileErrors. Added ErrorCode::Codegen (E700) + ErrorCode::Macro (E800). Replaced 30+ CString::new().unwrap() with cstr_owned(). Macro errors now visible to users.

---
## v0.339.0 — Stage 18.71 (P0 Typeck Enhancement)

Fixed 5 critical typeck deficiencies: type mismatch in let/return/if-branches, trait impl signature validation, void fn return value check. 106 tests flipped from compile_ok to compile_error.

---
## Earlier Versions

See git history for v0.260.0 through v0.338.0 release notes.
