# Stage 9.10 开发计划: Error recovery conformance 扩展

> **阶段**: Stage 9.10 (Stage 9 第 10 个子阶段)
> **版本**: v0.16.8 → v0.16.9
> **状态**: 🟡 In Progress
> **流程**: stage-committee-process.md v3.21 §13.4 + §17.1/§17.2/§17.3 + §1.2 验收

## 1. 背景

Stage 9.9 完成 conformance 437 → 497 (modules category, over 4/5!). Stage 9.10
继续扩展 **error recovery** 类别 (per `17-conformance-suite.md` §2 +
`02-grammar.md` §2 — error recovery via synthetic node).

## 2. §13.4 设计对齐

查阅:
- `docs/lang-design/02-grammar.md` §2 (Parser error recovery: "错误恢复通过
  synthetic node 实现：遇到错误时插入一个虚拟节点 + 跳过至下一个 `;` 或 `}` 继续 parse")
- `docs/lang-design/16-diagnostics.md` (error diagnostics spec)
- 前面 Stage 9.1-9.9 的 conformance 测试中已发现的 parser error/recovery 行为

## 3. 测试设计 (50 个 .lin tests)

### 3.1 Lexer errors (10 tests)

| 测试文件 | 描述 |
|---------|------|
| err_lex_empty_hex.lin | `FAIL: 0x` (empty hex — existing) |
| err_lex_empty_oct.lin | `FAIL: 0o` (empty octal) |
| err_lex_empty_bin.lin | `FAIL: 0b` (empty binary) |
| err_lex_unterminated_string.lin | `FAIL: "hello` (unterminated string) |
| err_lex_unterminated_char.lin | `FAIL: 'a` (unterminated char) |
| err_lex_invalid_escape.lin | `FAIL: '\\q'` (invalid escape) |
| err_lex_unterminated_block_comment.lin | `FAIL: /* unclosed` (unterminated block comment) |
| err_lex_invalid_unicode_escape.lin | `FAIL: "\\u{ZZ}"` (invalid unicode escape) |
| err_lex_leading_zero.lin | `FAIL: 007` (leading zero — already known) |
| err_lex_float_double_dot.lin | `FAIL: 1.2.3` (double dot in float) |

### 3.2 Parser errors — expressions (10 tests)

| 测试文件 | 描述 |
|---------|------|
| err_parse_unmatched_paren.lin | `FAIL: (1 + 2` (unmatched paren) |
| err_parse_unmatched_bracket.lin | `FAIL: arr[0` (unmatched bracket) |
| err_parse_unmatched_brace.lin | `FAIL: { 1` (unmatched brace) |
| err_parse_missing_semi.lin | `FAIL or PASS: let x = 1 let y = 2` (missing semicolon) |
| err_parse_double_semi.lin | `PASS or FAIL: let x = 1;;` (double semicolon) |
| err_parse_missing_expr.lin | `PASS: let x = ;` (missing expr — recovery) |
| err_parse_missing_type.lin | `PASS: let x: = 1;` (missing type — recovery) |
| err_parse_missing_pat.lin | `FAIL: let = 1;` (missing pattern) |
| err_parse_missing_fn_body.lin | `FAIL: fn f()` (missing fn body) |
| err_parse_missing_fn_name.lin | `FAIL: fn () {}` (missing fn name) |

### 3.3 Parser errors — items (10 tests)

| 测试文件 | 描述 |
|---------|------|
| err_parse_missing_struct_name.lin | `FAIL: struct {}` (missing struct name) |
| err_parse_missing_enum_name.lin | `FAIL: enum {}` (missing enum name) |
| err_parse_missing_trait_name.lin | `FAIL: trait {}` (missing trait name) |
| err_parse_missing_impl_type.lin | `FAIL: impl {}` (missing impl type) |
| err_parse_missing_const_name.lin | `FAIL: const : i32 = 0;` (missing const name) |
| err_parse_missing_const_type.lin | `FAIL: const C = 0;` (missing const type) |
| err_parse_missing_const_value.lin | `FAIL: const C: i32;` (missing const value) |
| err_parse_missing_where_colon.lin | `FAIL: fn f<T>() where T Clone {}` (missing colon in where) |
| err_parse_missing_arrow.lin | `FAIL: fn f() -> { 1 }` (missing return type after arrow) |
| err_parse_missing_use_path.lin | `PASS: use ;` (missing use path — recovery) |

### 3.4 Parser errors — types & patterns (8 tests)

| 测试文件 | 描述 |
|---------|------|
| err_parse_unclosed_array_type.lin | `FAIL: let x: [i32; = ...;` (unclosed array type) |
| err_parse_unclosed_tuple_type.lin | `FAIL: let x: (i32, = ...;` (unclosed tuple type) |
| err_parse_unclosed_generic.lin | `PASS: struct S<T { x: T }` (unclosed generic — recovery) |
| err_parse_unclosed_tuple_pat.lin | `FAIL: let (a, b = (1, 2);` (unclosed tuple pattern) |
| err_parse_unclosed_array_pat.lin | `FAIL: let [a, b = arr;` (unclosed array pattern) |
| err_parse_missing_pat_after_at.lin | `FAIL: let x @ = 1;` (missing pattern after @) |
| err_parse_missing_match_arm.lin | `PASS or FAIL: match x { }` (empty match — may be recovery) |
| err_parse_missing_match_arrow.lin | `FAIL: match x { 1 1 }` (missing => in match arm) |

### 3.5 Parser recovery — synthetic node (7 tests)

These test the parser's ability to recover from errors via synthetic node
insertion (per §2 of 02-grammar.md):

| 测试文件 | 描述 |
|---------|------|
| recovery_double_op.lin | `PASS: 1 + + 2` (double operator — recovery) |
| recovery_empty_let.lin | `PASS: let x = ;` (empty let init — recovery) |
| recovery_empty_attr.lin | `PASS: #[] fn f() {}` (empty attribute — recovery) |
| recovery_empty_generics.lin | `PASS: struct S<> {}` (empty generics — recovery) |
| recovery_empty_bound.lin | `PASS: fn f<T:>() {}` (empty bound — recovery) |
| recovery_empty_where.lin | `PASS: fn f<T>() where {}` (empty where — recovery) |
| recovery_unclosed_closure.lin | `PASS: \|x 1` (unclosed closure — recovery) |

### 3.6 Parser recovery — skip to next statement (5 tests)

These test the parser's ability to skip to the next `;` or `}` after an error:

| 测试文件 | 描述 |
|---------|------|
| recovery_skip_to_semi.lin | `PASS: fn f() { let x = 1 + +; let y = 2; }` (skip to next ;) |
| recovery_skip_to_brace.lin | `PASS: fn f() { let x = 1 + + { let y = 2; } }` (skip to next }) |
| recovery_multi_errors.lin | `PASS: fn f() { let x = ; let y = ; let z = 1; }` (multiple recoveries) |
| recovery_nested_errors.lin | `PASS: fn f() { if true { let x = ; } else { let y = ; } }` (nested recovery) |
| recovery_after_error.lin | `PASS: fn f() { let x = 1 + + 2; let y = 3; }` (continue after error) |

**累计**: 10 + 10 + 10 + 8 + 7 + 5 = **50 tests**

## 4. 验收标准

- ✅ `cargo clean && cargo test`: 2207+ tests pass (期望 +11 verification tests = 2218)
- ✅ `cargo fmt --check`: clean
- ✅ `cargo clippy --all-targets`: 0 warnings
- ✅ `python3 tests/conformance/run_all.py`: 547 passed (497 + 50 new)
- ✅ §17.3 三阶段文档协议: plan + gate-review + test plan
- ✅ 0 regressions

## 5. 版本

- Cargo.toml: 0.16.8 → 0.16.9
- api-naming-standard.md: v2.12 → v2.13

---

**创建日期**: 2026-07-26
