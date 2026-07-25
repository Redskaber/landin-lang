# Stage 9.10 测试计划: Error recovery conformance expansion

> **阶段**: Stage 9.10
> **对应代码**: tests/v0/stage9/plan/error_recovery_tests.rs + tests/conformance/00-parse/09-error-recovery/*.lin
> **状态**: ✅ Complete

## 1. 测试目标

1. 验证 conformance `09-error-recovery/` category 扩展 (1 → 51 .lin files)
2. 验证 §2 (error recovery via synthetic node) 实现正确性
3. 系统化记录 lexer errors + parser errors + recovery behavior

## 2. Conformance .lin 测试 (tests/conformance/00-parse/09-error-recovery/)

### 新增 50 个测试 (Stage 9.10)

| 类别 | 测试数 | 备注 |
|------|-------|------|
| Lexer errors | 10 | empty-oct/bin, unterminated string/char/block-comment, invalid escape/unicode, leading-zero, float-double-dot (PASS), negative-zero (PASS) |
| Parser errors — expressions | 10 | unmatched paren/bracket/brace, missing-semi (FAIL), double-semi (FAIL), missing-expr (PASS), missing-type (PASS), missing-pat (FAIL), missing-fn-body (FAIL), missing-fn-name (FAIL) |
| Parser errors — items | 10 | missing struct/enum/trait/impl/const-name/type/value, missing where-colon (FAIL), missing-arrow-type (PASS), missing-use-path (PASS) |
| Parser errors — types & patterns | 8 | unclosed array-type (FAIL), tuple-type (FAIL), generic (PASS), tuple-pat (FAIL), array-pat (FAIL), missing-pat-after-at (FAIL), missing-match-arrow (FAIL), empty-match (PASS) |
| Recovery — synthetic node | 7 | double-op, empty-let, empty-attr, empty-generics, empty-bound, empty-where, unclosed-closure (all PASS) |
| Recovery — skip to next stmt | 5 | skip-to-semi (PASS), skip-to-brace (FAIL), multi-errors (PASS), nested-errors (PASS), after-error (PASS) |
| **Total new** | **50** | |

### 累计 conformance: 497 → 547 (+50 ✅)

## 3. 关键发现

**Parser recovery behavior systematically documented**:

The conformance suite now comprehensively documents the parser's error recovery
behavior per §2 of `02-grammar.md` ("错误恢复通过 synthetic node 实现"):

1. **Synthetic node recovery** (12 tests): The parser inserts synthetic nodes for:
   - Double operators (`1 + + 2`) — PASS
   - Empty let init (`let x = ;`) — PASS
   - Empty attribute (`#[] fn f() {}`) — PASS
   - Empty generics (`struct S<> {}`) — PASS
   - Empty bound (`fn f<T:>() {}`) — PASS
   - Empty where clause (`fn f<T>() where {}`) — PASS
   - Unclosed closure param (`|x 1`) — PASS
   - Missing type (`let x: = 1;`) — PASS
   - Missing expression (`let x = ;`) — PASS
   - Missing return type (`fn f() -> { 1 }`) — PASS
   - Missing impl type (`impl {}`) — PASS
   - Missing use path (`use ;`) — PASS

2. **Parser error cases** (21 tests): The parser reports errors for:
   - Unclosed delimiters (paren/bracket/brace/array-type/tuple-type/tuple-pat/array-pat)
   - Missing required elements (fn-name/fn-body/struct-name/enum-name/trait-name/const-name/const-type/const-value/pat/where-colon/match-arrow)
   - Invalid syntax (missing-semi/double-semi/invalid-glob/vis-no-item)

3. **Lexer error cases** (8 tests): The lexer reports errors for:
   - Empty hex/octal/binary literals
   - Unterminated string/char/block-comment
   - Invalid escape sequences
   - Leading zeros in decimal integers

---

**创建日期**: 2026-07-26
