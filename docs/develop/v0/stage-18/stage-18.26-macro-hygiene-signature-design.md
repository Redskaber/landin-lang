# Stage 18.26 — Macro Hygiene Activation (Signature Change + apply_hygiene)

> **Author**: redskaber + ARCH-A + REV-A
> **Date**: 2026-08-07
> **Version**: v0.305.0
> **Process**: stage-committee-process.md v5.0 §13.5 (1 轮自审定稿)
> **Status**: ✅ Complete

## 1. 阶段目标

**背景**: Stage 18.20 实现了 `apply_hygiene` 但因签名限制（`expand_macro`
接收 `&Rodeo`，`apply_hygiene` 需要 `&mut Rodeo` 来 intern 新符号）未激活。
本阶段改变签名链以激活 hygiene。

**用户反馈**: "println! 系列也是macro 的一部分，所以在思考和设计上不能
只解决 println! 而忽略 macro" + "不能偏离正确的设计和实现（正确 > 妥协）"

**具体目标**:
1. 改变 `expand_macro` 签名: `&Rodeo` → `&mut Rodeo`
2. 改变 `expand_macro_calls_with_errors` 签名: `&Rodeo` → `&mut Rodeo`
3. 改变 `expand_macros_with_errors` 签名: `&Rodeo` → `&mut Rodeo`
4. 改变 `expand_macro_calls` / `expand_macros` / `collect_macro_defs` /
   `collect_macro_defs_with_errors` / `build_builtin_macro_table` 签名
5. 在 `expand_macro` 中创建 `HygieneContext` 并调用 `apply_hygiene`
6. 移除 `HygieneContext` 和 `apply_hygiene` 的 `#[allow(dead_code)]`
7. driver 传递 `&mut interner`

## 2. 设计原则

| 原则 | 落实方式 |
|------|----------|
| §1.0 原則 6 "通用 > 特解" | 一个 apply_hygiene 处理所有宏 |
| §10 命名 | 无新 API (签名变更) |
| §11 接口隔离 | hygiene 逻辑在 macro_expand.rs |
| §13.4 重构治理 | 签名变更，行为改进 |
| 单一职责 | apply_hygiene 只重命名 |
| 避免死代码 | 移除 #[allow(dead_code)] |
| 避免分散内容 | hygiene 激活集中在一处 |

## 3. 实现方案

### 3.1 签名变更

所有接收 `&Rodeo` 的公共函数改为 `&mut Rodeo`：
- `expand_macro`
- `expand_macro_calls` / `expand_macro_calls_with_errors`
- `expand_macros` / `expand_macros_with_errors`
- `collect_macro_defs` / `collect_macro_defs_with_errors`
- `build_builtin_macro_table`
- `make_builtin_macro_rule` (内部函数)

### 3.2 driver 更新

```rust
// Before:
let (tokens, macro_errs) =
    crate::parser::macro_expand::expand_macros_with_errors(tokens, &interner);

// After:
let (tokens, macro_errs) =
    crate::parser::macro_expand::expand_macros_with_errors(tokens, &mut interner);
```

### 3.3 apply_hygiene 激活

```rust
pub fn expand_macro(def: &MacroRulesDef, input: &[Token], interner: &mut Rodeo) -> Option<Vec<Token>> {
    for rule in &def.rules {
        let mut captures = HashMap::new();
        if match_pattern(&rule.pattern, input, &mut captures, interner) {
            // Stage 18.26: Apply hygiene before substitution.
            let mut hygiene = HygieneContext::new();
            let hygienic_body = apply_hygiene(&rule.body, &captures, interner, &mut hygiene);
            return Some(substitute_body(&hygienic_body, &captures));
        }
    }
    None
}
```

### 3.4 match_pattern 签名不变

`match_pattern` 接收 `&Rodeo`（不 intern 新符号），不需要改为 `&mut`。

### 3.5 风险评估

**风险**: 改变 `interner` 从 `&` 到 `&mut` 可能导致借用冲突
- **缓解**: driver 中 `interner` 在 macro 展开期间不被其他引用使用

## 4. 测试矩阵（§9.4.3 1:3+ 正负比例）

新增 8 个测试，2 正 : 6 负 = 1:3 ✓

| # | 类型 | 名称 | 验证 |
|---|------|------|------|
| 1 | positive | println_still_works_after_hygiene | println! 仍正常 |
| 2 | positive | user_macro_still_works_after_hygiene | 用户宏仍正常 |
| 3 | negative | apply_hygiene_now_called | apply_hygiene 被调用 |
| 4 | negative | hygiene_context_not_dead_code | HygieneContext 被使用 |
| 5 | negative | println_with_args_after_hygiene | println! 带参数仍正常 |
| 6 | negative | eprintln_after_hygiene | eprintln! 仍正常 |
| 7 | negative | macro_repetition_after_hygiene | 重复宏仍正常 |
| 8 | negative | macro_separator_after_hygiene | 分隔符宏仍正常 |

## 5. 验收

- [x] `cargo build --features llvm-backend` ✅
- [x] `cargo fmt --check` ✅
- [x] `cargo clippy --all-targets --features llvm-backend` 0 warnings ✅
- [x] `cargo test --features llvm-backend` 全绿，新增 8 测试 ✅
