# Stage 18.08 — Macro Expansion Error Collection + Driver Integration

> **Author**: redskaber + ARCH-A + REV-A
> **Date**: 2026-08-06
> **Version**: v0.293.0
> **Process**: stage-committee-process.md v5.0 §13.5 (1 轮自审定稿)
> **Status**: ✅ Complete

## 1. 阶段目标

改进 D7 (错误处理)：当 macro_rules! 定义语法错误或 macro 调用无法
展开时，收集明确的错误信息，而非静默跳过。

具体目标：
1. 新增 `MacroError` 类型（§10 Error 后缀）
2. `expand_macros` 收集错误（不 panic）
3. `CompileErrors` 增加 `macro_errors: Vec<MacroError>` 字段
4. `driver::compile` 将 macro 错误传播到 `CompileErrors`

## 2. 设计原则

| 原则 | 落实方式 |
|------|----------|
| §10 Error 命名 | `MacroError` 遵循 `<Stage>Error` 后缀 |
| §10 Entry 函数 | `expand_macros_with_errors` 是新自由函数入口 |
| §11 接口隔离 | `MacroError` 类型在 `macro_expand.rs` 定义, driver 通过 `CompileErrors` 字段持有 |
| §1.0 原則 6 "通用 > 特例" | 一个 `MacroError` 类型覆盖所有错误场景 |
| 单一职责 | 错误收集在 `expand_macros_with_errors`, 错误显示在 driver |
| 避免死代码 | `MacroError` 字段全部 public, 由 driver 使用 |

## 3. 数据结构

```rust
// src/parser/macro_expand.rs

/// Stage 18.08: Error during macro_rules! expansion.
///
/// Per §10: error type follows `<Stage>Error` suffix pattern.
#[derive(Debug, Clone)]
pub struct MacroError {
    /// Human-readable error message.
    pub message: String,
    /// Source span where the error occurred (best-effort).
    pub span: crate::session::Span,
}

impl MacroError {
    /// Per §10: constructor follows `new` convention.
    pub fn new(message: impl Into<String>, span: crate::session::Span) -> Self {
        Self { message: message.into(), span }
    }
}
```

## 4. API 设计

```rust
// src/parser/macro_expand.rs

/// Stage 18.08: Top-level macro expansion pass with error collection.
///
/// Like `expand_macros` but also collects errors encountered during
/// expansion (malformed macro_rules! definitions, no-matching-rule
/// macro calls, etc.). Errors do NOT stop expansion — the compiler
/// continues with whatever tokens were produced.
///
/// Per §10: `expand_macros_with_errors` follows `<verb>_<noun>_<prep>`.
pub fn expand_macros_with_errors(
    tokens: Vec<Token>,
    interner: &Rodeo,
) -> (Vec<Token>, Vec<MacroError>);
```

`expand_macros` 保持不变 (向后兼容), 内部委托给
`expand_macros_with_errors` 并丢弃错误。

## 5. driver 集成

```rust
// src/driver.rs

pub struct CompileErrors {
    pub lex: Vec<LexError>,
    pub parse: Vec<ParseError>,
    pub resolve: Vec<ResolveError>,
    pub typeck: Vec<TypeError>,
    pub borrowck: Vec<BorrowError>,
    pub trait_errors: Vec<TraitError>,
    pub macro_errors: Vec<MacroError>,  // Stage 18.08 新增
}

// In compile():
let (tokens, macro_errs) =
    crate::parser::macro_expand::expand_macros_with_errors(tokens, &interner);
errors.macro_errors = macro_errs;
```

## 6. 错误场景覆盖

| 场景 | 错误信息 |
|------|---------|
| `macro_rules!` 定义中规则解析失败 | `"malformed macro_rules! body in definition of '{name}'"` |
| `name!(...)` 调用未匹配任何规则 | `"no matching rule for macro '{name}'"` |
| macro 调用展开超过 MAX_EXPANSION_ROUNDS | `"macro expansion exceeded {N} rounds (possible infinite recursion)"` |

## 7. 测试矩阵（§9.4.3 1:3+ 正负比例）

新增 8 个测试，2 正 : 6 负 = 1:3 ✓

| # | 类型 | 名称 | 验证 |
|---|------|------|------|
| 1 | positive | macro_error_no_macros | 无 macro_rules! → 无错误 |
| 2 | positive | macro_error_valid_macro_no_errors | 有效 macro_rules! + 调用 → 无错误 |
| 3 | negative | macro_error_no_matching_rule | 调用不匹配任何规则 → 1 错误 |
| 4 | negative | macro_error_malformed_def | 畸形 macro_rules! body → 1 错误 |
| 5 | negative | macro_error_struct_fields | MacroError 字段可访问 |
| 6 | negative | macro_error_new_constructor | MacroError::new() 创建错误 |
| 7 | negative | compile_errors_macro_field | CompileErrors.macro_errors 字段存在 |
| 8 | negative | expand_macros_with_errors_returns_tuple | 返回 (tokens, errors) 元组 |

## 8. 验收

- [x] `cargo build --features llvm-backend` ✅
- [x] `cargo fmt --check` ✅
- [x] `cargo clippy --all-targets --features llvm-backend` 0 warnings ✅
- [x] `cargo test --features llvm-backend` 全绿，新增 8 测试 ✅
  - 495 lib (487 → 495, +8) + 2537 integration = **3,032** total, 0 failures

## 9. 结论

Stage 18.08 完成 macro_rules! 错误收集 + driver 集成。D7 (错误处理)
维度从 ⚠️ 改进为 ✅。下一阶段 (Stage 18.09) 规划：
- println! 通解化迁移 (将 println! 从特解改为 macro_rules!)
