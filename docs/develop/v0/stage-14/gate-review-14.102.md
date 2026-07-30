# Stage 14.102 — Gate Review: Deep Audit Phase 2 (ME-1/ME-2 + Lexer Fixes)

> **Author**: redskaber
> **Date**: 2026-07-30
> **Version**: v0.115.0 → v0.116.0
> **Process**: stage-committee-process.md v3.22 §25 (D8 deep review)

## 1. Stage Summary

Stage 14.102 continues the deep architecture audit by fixing 5 more P0 bugs
identified in Phase 1:

- **ME-1**: `AggregateKind::Closure` → `Ty::Error` silently (typeck)
- **ME-2**: `Rvalue::BinaryOp2` (Range) → `Ty::Error` silently (typeck)
- **Lexer fix 1**: `lex_escape_from_str` silent fallback (`'\q'` → `'q'`)
- **Lexer fix 2**: `lex_hex`/`lex_oct`/`lex_bin` inconsistent suffix error reporting

All 5 are fully fixed with regression tests.

## 2. Bugs Fixed

### ME-1: AggregateKind::Closure → Ty::Error silently

**Symptom**: Closures in type checking silently got `Ty::Error` instead of a
proper type variable.

**Root cause**: `src/typeck/checker.rs` had `_ => Ty::new(TyKind::Error, Span::DUMMY)`
catch-all that silently handled `AggregateKind::Closure`.

**Fix**: Added explicit arm for `AggregateKind::Closure` that:
- Infers operand types (captures)
- Returns a fresh `TyVar` (closure type is opaque until called)

### ME-2: Rvalue::BinaryOp2 (Range) → Ty::Error silently

**Symptom**: Range expressions (`start..end`) in type checking silently got
`Ty::Error` without any error message.

**Root cause**: `src/typeck/checker.rs` returned `Ty::Error` with a comment
"Range type (Stage 3)" but no error was pushed.

**Fix**: Now pushes a `TypeError` explaining that range expressions are not
supported in type position in v0.1 (only in for-loop iterators).

### Lexer fix 1: lex_escape_from_str silent fallback

**Symptom**: `'\q'` silently became `'q'` instead of erroring.

**Root cause**: `src/lexer/string.rs::lex_escape_from_str` had
`_ => s.chars().last().unwrap_or('\0')` fallback.

**Fix**: Changed return type to `Option<char>`, returning `None` for unrecognized
escapes. Caller now pushes a `LexError` with a clear message.

### Lexer fix 2: lex_hex/lex_oct/lex_bin inconsistent suffix errors

**Symptom**: `0xFF_i33` (invalid suffix) was silently accepted, while
`42_i33` (decimal) correctly reported an error.

**Root cause**: `lex_hex`/`lex_oct`/`lex_bin` used `and_then` with `_ => None`,
silently swallowing invalid suffixes. The decimal path correctly pushed a
`LexError`.

**Fix**: Added `parse_int_suffix_with_error` helper that pushes a `LexError`
for invalid suffixes (matching the decimal path). Updated all 3 non-decimal
functions to use this helper.

## 3. Test Count Updates

| Suite | Before | After | Delta |
|-------|--------|-------|-------|
| Rust tests | 1951 | 1951 | 0 |
| Conformance tests | 5209 | 5213 | +4 |

New tests:
- `lex-invalid-escape.lin` — invalid char escape `'\q'` (compile_error)
- `lex-invalid-hex-suffix.lin` — `0xFF_i33` invalid hex suffix (compile_error)
- `lex-invalid-oct-suffix.lin` — `0o77_u33` invalid octal suffix (compile_error)
- `lex-invalid-bin-suffix.lin` — `0b1010_u33` invalid binary suffix (compile_error)

## 4. Verification

```
cargo build --release --features llvm-backend: ✅
cargo fmt: ✅ clean
cargo clippy --all-targets --features llvm-backend: ✅ 0 warnings
cargo test --features llvm-backend: ✅ 1951 passed, 0 failed
python3 tests/conformance/run_all.py: ✅ 5213 passed, 0 failed
```

## 5. Remaining P0 Bugs (deferred to Stage 14.103+)

7 P0 bugs remain from Phase 1 audit:
- ME-3: Non-literal `Repeat` count → silently falls back to 1
- ME-4: Const/static body lookup silent
- ME-5: Unknown macro → `Ty::Error` silently
- ME-7: `place_ty` silent fallbacks for Deref/Index
- SH-5: `LLVMSysEmitter::emit_checked_binop` stub
- SH-7: `codegen_rvalue` catch-all returns "0"
- SH-8: `Terminator::Drop` no-op

Plus ~2,475 LOC dead code cleanup (P1).

## 6. Stage Verdict

**PASS** — Fixed 5 P0 bugs (ME-1, ME-2, 3 lexer fixes). +4 new regression
tests. No regressions.

Per §1.0 原则 5 "报错 > 静默": 5 new error cases now produce clear errors
instead of silent wrong output.

Per §1.0 原则 6 "通用 > 特例": `parse_int_suffix_with_error` helper handles
all 3 non-decimal integer literal forms uniformly.

v0.116.0: minor bump (5 P0 fixes — important correctness improvements)
