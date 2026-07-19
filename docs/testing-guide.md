# Landin Stage 0 测试指南

> **范围**：Stage 0 前端（Lexer + Parser + AST）测试方法论
> **测试规模**：375 个（lexer 109 + parser 85 + ast_structure 149 + hir_structure 20 + lib 12）
> **最后更新**：Stage 1.1（2025）

---

## 1. 如何运行测试

### 1.1 运行全部测试

```bash
cd /path/to/landin-stage0
cargo test
```

预期输出：
```
running 79 tests
test test_int_dec_basic ... ok
...
test result: ok. 79 passed; 0 failed; 0 ignored

running 80 tests
test test_fn_empty ... ok
...
test result: ok. 80 passed; 0 failed; 0 ignored

running 28 tests
test test_ast_fn_item_structure ... ok
...
test result: ok. 28 passed; 0 failed; 0 ignored

Doc-tests: 0 passed; 0 failed
```

### 1.2 运行单个测试文件

```bash
cargo test --test lexer           # 仅运行 lexer 测试
cargo test --test parser          # 仅运行 parser 测试
cargo test --test ast_structure   # 仅运行 AST 结构断言测试
```

### 1.3 运行单个测试

```bash
cargo test test_int_dec_basic           # 按名称匹配
cargo test --test lexer test_int_dec_   # 名称前缀匹配
cargo test --test parser test_fn_       # 名称前缀匹配
```

### 1.4 显示 println 输出

默认 Rust 测试捕获 `println!` 输出。需查看时加 `--nocapture`：

```bash
cargo test -- --nocapture
```

### 1.5 多线程 vs 单线程

默认多线程。调试 race condition 或 stack trace 时强制单线程：

```bash
cargo test -- --test-threads=1
```

### 1.6 仅运行被忽略的测试

```bash
cargo test -- --ignored
```

### 1.7 性能基准（不适用 Stage 0）

Stage 0 不使用 criterion/bench。性能测试推迟到月 7+ codegen 阶段。

---

## 2. 测试文件组织

### 2.1 文件结构

```
landin-stage0/
├── tests/
│   ├── lexer.rs           # Lexer 集成测试（79 个）
│   ├── parser.rs          # Parser 集成测试（80 个）
│   └── ast_structure.rs   # AST 结构断言测试（28 个）
└── src/
    └── ...                # 库代码（内部测试可通过 #[cfg(test)] mod tests）
```

### 2.2 文件职责

| 文件 | 职责 | 测试类型 |
|---|---|---|
| `tests/lexer.rs` | 验证 lexer 输出的 `TokenKind` 正确性 | 精确 token 断言 + 模式断言 |
| `tests/parser.rs` | 验证 parser 不报错 / 报错行为 | smoke test（`assert_no_errors` / `assert_has_errors`） |
| `tests/ast_structure.rs` | 验证 parser 产生的 AST 节点结构正确 | 结构断言（`match` + `assert_eq!`）+ P0 回归测试 |

### 2.3 不使用内部测试

Stage 0 不在 `src/` 内部写 `#[cfg(test)] mod tests`。所有测试为集成测试（`tests/` 目录），通过 `landin_compiler` crate 的公开 API 测试。

### 2.4 Conformance 套件（未实现）

蓝图 §17 要求 5000 个 conformance 测试，目录结构 `tests/conformance/00-parse/...`。Stage 0 前端不实现 conformance runner，推迟到月 3+ HIR 阶段统一建设。

---

## 3. 如何编写新测试

### 3.1 Lexer 测试模板

```rust
// tests/lexer.rs

#[test]
fn test_my_new_token() {
    // 方式 1：精确断言（推荐）
    assert_eq!(
        lex("my_input"),
        vec![TokenKind::MyExpectedToken, TokenKind::Eof]
    );
    
    // 方式 2：模式断言（用于带数据的 token，如 IntLit）
    let tokens = lex("42");
    assert!(matches!(tokens[0], TokenKind::IntLit(42, None)));
}

// 错误场景测试
#[test]
fn test_my_invalid_input_reports_error() {
    let (_tokens, errors) = {
        let mut interner = Rodeo::new();
        landin_compiler::lexer::tokenize("invalid input", &mut interner)
    };
    assert!(!errors.is_empty(), "should report error");
}
```

### 3.2 Parser 测试模板

```rust
// tests/parser.rs

#[test]
fn test_my_new_construct_parses() {
    // 方式 1：仅检查无错误（smoke test，最低标准）
    assert_no_errors("fn f() { my_construct; }");
}

#[test]
fn test_my_invalid_construct_errors() {
    // 方式 2：检查有错误
    assert_has_errors("fn f() { invalid_syntax }");
}
```

### 3.3 AST 结构断言测试模板（推荐）

```rust
// tests/ast_structure.rs

#[test]
fn test_my_construct_ast_structure() {
    let (krate, errors) = parse("fn f(x: i32) -> i32 { x }");
    assert!(errors.is_empty());
    
    let fn_decl = match &krate.items[0].kind {
        ItemKind::Fn(f) => f,
        _ => panic!("expected Fn item"),
    };
    
    // 验证签名
    assert_eq!(fn_decl.sig.inputs.len(), 1, "should have 1 param");
    match &fn_decl.sig.inputs[0].ty {
        Ty::Int(IntTy::I32, _) => {} // correct
        other => panic!("expected Ty::Int(I32), got {:?}", other),
    }
    match &fn_decl.sig.output {
        FnRetTy::Ty(Ty::Int(IntTy::I32, _)) => {} // correct
        other => panic!("expected FnRetTy::Ty(Int(I32)), got {:?}", other),
    }
    
    // 验证 body
    let body = fn_decl.body.as_ref().expect("body");
    match &body.expr {
        Some(expr) => match expr.as_ref() {
            Expr::Path(_, _, _) => {} // correct
            other => panic!("expected Path expr, got {:?}", other),
        },
        None => panic!("expected trailing expr"),
    }
}
```

### 3.4 P0 回归测试模板

```rust
// tests/ast_structure.rs

#[test]
fn test_regression_my_p0_fix() {
    // 描述 P0 缺陷
    // 修复前：<observed bad behavior>
    // 修复后：<expected good behavior>
    let (krate, errors) = parse("fn f() { my_fixed_construct }");
    assert!(errors.is_empty(), "should parse without errors: {:?}", errors);
}
```

---

## 4. 测试命名规范

### 4.1 命名模式

```
test_<category>_<feature>[_<variant>]
```

### 4.2 分类前缀

| 前缀 | 适用范围 | 示例 |
|---|---|---|
| `test_int_` | 整数字面量 | `test_int_dec_basic` / `test_int_hex` / `test_int_oct` |
| `test_float_` | 浮点字面量 | `test_float_basic` / `test_float_exp` / `test_float_suffix_f32` |
| `test_char_` | 字符字面量 | `test_char_basic` / `test_char_escape_newline` |
| `test_string_` | 字符串字面量 | `test_string_basic` / `test_string_escape` |
| `test_byte_` | byte 字面量 | `test_byte_literal` / `test_byte_escape_hex` |
| `test_raw_string_` | raw string | `test_raw_string_basic` / `test_raw_string_hash` |
| `test_bool_` | 布尔字面量 | `test_bool_true` / `test_bool_false` |
| `test_op_` | 运算符 | `test_op_arithmetic` / `test_op_comparison` / `test_op_maximal_munch_*` |
| `test_kw_` | 关键字 | `test_kw_strict_core` / `test_kw_async_await` |
| `test_ident_` | 标识符 | `test_ident_basic` / `test_ident_unicode` |
| `test_lifetime_` | 生命周期 | `test_lifetime_basic` |
| `test_comment_` | 注释 | `test_comment_line` / `test_comment_nested_block` |
| `test_error_` | 错误场景 | `test_error_missing_semicolon` / `test_error_bad_char_continues` |
| `test_punct_` | 标点 | `test_punct_brackets` / `test_punct_dot` |
| `test_fn_` | 函数声明 | `test_fn_empty` / `test_fn_with_params` |
| `test_struct_` | 结构体 | `test_struct_named` / `test_struct_tuple` |
| `test_enum_` | 枚举 | `test_enum_unit_variants` / `test_enum_tuple_variants` |
| `test_trait_` | trait | `test_trait_decl` |
| `test_impl_` | impl 块 | `test_impl_inherent` |
| `test_const_` / `test_static_` | 常量/静态变量 | `test_const_static` |
| `test_use_` | use 声明 | `test_use_decl` |
| `test_type_` | 类型 | `test_type_ref` / `test_type_array` |
| `test_if_` / `test_match_` / `test_loop_` / `test_while_` / `test_for_` | 控制流 | `test_if_else` / `test_match_with_guard` |
| `test_return_` / `test_break_` / `test_continue_` | 跳转 | `test_return_expr` |
| `test_unary_` / `test_assign_` / `test_cast_` / `test_try_` / `test_range_` | 表达式 | `test_unary_neg` / `test_cast` |
| `test_ast_` | AST 结构断言 | `test_ast_fn_item_structure` / `test_ast_binop_precedence_structure` |
| `test_regression_` | P0 回归 | `test_regression_break_keyword` / `test_regression_closure_empty_params` |
| `test_edge_` | 边界 case | `test_edge_empty_file` / `test_edge_deeply_nested_blocks` |
| `test_fib_program` / `test_struct_with_impl` | 复杂程序 | 整体程序不强制前缀 |

### 4.3 命名约定

- 全小写 + 下划线分隔（snake_case）
- 描述行为而非实现：`test_int_dec_basic` ✅ 而非 `test_lex_number_decimal` ❌
- 错误场景用 `_error_` 或 `_errors` 后缀：`test_error_missing_semicolon`
- 多变体用 `_basic` / `_with_*` / `_edge`：`test_int_dec_basic` / `test_int_dec_with_suffix`

---

## 5. Conformance 测试格式

对照蓝图 `17-conformance-suite.md`。

### 5.1 Stage 0 现状

Stage 0 前端**不实现** conformance runner，仅使用集成测试。Conformance 套件推迟到月 3+ HIR 阶段统一建设。

### 5.2 蓝图要求的目录结构

```
tests/
├── conformance/
│   ├── 00-parse/                          # Parse 测试（600 个）
│   │   ├── 00-literals/
│   │   │   ├── 001-integer-dec.lin
│   │   │   └── ...
│   │   ├── 01-operators/
│   │   ├── 02-control-flow/
│   │   ├── 03-patterns/
│   │   ├── 04-types/
│   │   ├── 05-attributes/
│   │   └── 99-error-recovery/
│   ├── 01-typecheck/                      # 月 4+
│   ├── 02-borrowck/                       # 月 6+
│   ├── 03-codegen/                        # 月 7+
│   ├── 04-e2e/                            # 月 9+
│   ├── 05-soundness/                      # 月 6+
│   ├── 06-stdlib/                         # 月 10+
│   ├── 07-integration/                    # 月 11+
│   ├── run_all.py                         # 主 runner
│   └── expected/                          # 期望输出快照
└── fuzz/                                  # 月 12+
```

### 5.3 测试用例格式（蓝图 §3）

每个 `.lin` 测试文件包含 header 注释 + Landin 源码：

```landin
// tests/conformance/00-parse/00-literals/001-integer-dec.lin
// CATEGORY: parse
// DESCRIPTION: Decimal integer literals
// EXPECTED: compile_ok

fn main() {
    let a: i32 = 42;
    let b: i32 = 0;
    let c: i64 = 1_000_000;
    let d: u8 = 255;
}
```

错误用例：

```landin
// tests/conformance/01-typecheck/99-error-cases/E0308-mismatched-types.lin
// CATEGORY: typecheck
// DESCRIPTION: E0308 mismatched types
// EXPECTED: compile_error
// ERROR_CODE: E0308
// ERROR_PATTERN: expected `i32`, found `&str`

fn main() {
    let x: i32 = "hello";   // E0308
}
```

### 5.4 Header 字段完整清单

| 字段 | 必需 | 说明 |
|---|---|---|
| `CATEGORY` | ✅ | parse / typecheck / borrowck / codegen / e2e / soundness / stdlib / integration |
| `DESCRIPTION` | ✅ | 测试描述 |
| `EXPECTED` | ✅ | `compile_ok` / `compile_error` / `run_ok` / `run_panic` |
| `ERROR_CODE` | compile_error 时 | E0xxx |
| `ERROR_PATTERN` | compile_error 时 | 正则 |
| `EXPECTED_STDOUT` | run_ok 时可选 | 期望 stdout |
| `EXPECTED_EXIT_CODE` | run_ok 时可选 | 期望退出码 |
| `PANIC_PATTERN` | run_panic 时必需 | panic 信息正则 |
| `DUMP_MIR` | 可选 | true 时 dump MIR |
| `DUMP_LLVM_IR` | 可选 | true 时 dump LLVM IR |
| `REFERENCES` | 可选 | 相关文档章节 |
| `IGNORE` | 可选 | true 时跳过（含原因） |

### 5.5 Stage 0 集成测试 → Conformance 迁移计划

月 3 启动时，建议：

1. 建立 `tests/conformance/00-parse/` 目录结构
2. 将现有 187 个集成测试拆分迁移到对应子目录
3. 实现 `run_all.py` Python runner（蓝图 §4.1 给出框架）
4. 补足到 600 个 parse 测试（蓝图 §2 要求）

---

## 6. 测试质量标准

### 6.1 当前测试质量

| 维度 | 当前状态 | 目标 |
|---|---|---|
| Token 精确断言 | 88 处 | 维持 |
| Token 模式断言 | 45 处 | 维持 |
| AST 结构断言 | 8 处 | ≥ 50 处（覆盖 28 Expr + 16 Ty + 12 Pat） |
| Span 正确性断言 | 0 处 | ≥ 10 处 |
| 错误信息内容断言 | 0 处 | ≥ 5 处 |
| 错误检测（has_errors） | 13 处 | ≥ 20 处 |

### 6.2 推荐的测试金字塔

```
       /\
      /  \   Conformance（蓝图 §17，月 3+）
     /----\
    /      \ AST 结构断言（推荐扩展）
   /--------\
  /          \ Smoke test（当前 80%）
 /____________\
  Lexer 精确断言
```

### 6.3 测试评审清单

新测试合并前应自查：

- [ ] 命名遵循 §4 规范
- [ ] 测试独立（不依赖其他测试执行顺序）
- [ ] 至少一个 `assert!` 或 `assert_eq!`
- [ ] 错误信息清晰（`assert!(cond, "message: {:?}", actual)`）
- [ ] 覆盖正常 + 边界 + 错误三类场景（如适用）
- [ ] 不使用 `unwrap()` 在测试体（用 `expect("reason")` 代替）
- [ ] AST 结构断言覆盖到关键 variant（推荐）

---

**Landin Stage 0 测试指南 — 完**
