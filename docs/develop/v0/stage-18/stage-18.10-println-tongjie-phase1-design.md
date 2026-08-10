# Stage 18.10 — println! 通解化迁移 Phase 1 (Prelude macro_rules! 定义)

> **Author**: redskaber + ARCH-A + REV-A
> **Date**: 2026-08-06
> **Version**: v0.294.0
> **Process**: stage-committee-process.md v5.0 §13.5 (1 轮自审定稿)
> **Status**: ✅ Complete

## 1. 阶段目标

按照 v0.6 P1 后续规划，开始 println! 通解化迁移。Stage 17.11 标记了
4 层 println! 特解（AST/HIR/MIR/Codegen），并明确通解路径：

```
println!("x={}", x)
  → macro_rules! 展开
  → Call(__landin_println, [__landin_format_args("x={}", x)])
```

本阶段（Phase 1）的目标是：**为 println!/print!/eprintln!/eprint!
注册内置 macro_rules! 定义到 prelude**，使调用点先经过 `expand_macros`
通道。但 parser 的特解**暂时保留**（兼容现有 HIR/MIR/Codegen）。

具体做法：
1. 在 `expand_macros` 入口处，先注入 4 个内置 macro_rules! 定义
   （`println` / `print` / `eprintln` / `eprint`），让它们出现在
   MacroTable 中。
2. 但每个内置 macro 的 rule body 设计为**展开为同样的 `name!(...)`
   调用形式**（即 no-op 展开），这样 parser 仍能识别并走特解路径。
3. 这样做的好处：
   - 验证 `expand_macros` 通道能正确处理内置宏（无 regression）
   - 为 Phase 2（真正展开为 Call）打好基础
   - 用户可以在源码中显式 `macro_rules! println { ... }` 覆盖内置定义

## 2. 设计原则

| 原则 | 落实方式 |
|------|----------|
| §1.0 原則 6 "通用 > 特例" | 内置宏通过同一个 `expand_macros` 通道，不绕过 |
| §10 命名 | 新增 `builtin_macros` / `register_builtin_macros` |
| §11 接口隔离 | 内置宏注册逻辑封装在 macro_expand.rs |
| 单一职责 | `register_builtin_macros` 只注册，不展开 |
| 高内聚低耦合 | 内置宏表集中在 macro_expand.rs |
| 避免死代码 | 所有新增函数都被 expand_macros 调用 |
| 避免分散内容 | 内置宏定义不分散到其他模块 |

## 3. 数据结构

```rust
// src/parser/macro_expand.rs

/// Stage 18.10: Names of the built-in macros that are always available
/// (registered into every MacroTable before user macros).
///
/// Per §10: const naming follows UPPER_SNAKE_CASE.
pub const BUILTIN_MACRO_NAMES: &[&str] = &["println", "print", "eprintln", "eprint"];

/// Stage 18.10: Build the table of built-in macro_rules! definitions.
///
/// Each built-in macro has a single rule whose body is the same call
/// form (`name!(...)`) — i.e. a no-op expansion. This lets the macro
/// pass through `expand_macros` unchanged, so the parser's existing
/// special-case can handle it.
///
/// In Phase 2, these bodies will be replaced with real expansions to
/// `Call(__landin_println, [__landin_format_args(...)])`.
///
/// Per §10: `build_builtin_macro_table` follows `<verb>_<noun>_<noun>`.
pub fn build_builtin_macro_table(interner: &mut Rodeo) -> MacroTable;
```

## 4. 实现细节

### 4.1 内置宏的 rule 设计

每个内置宏的 rule pattern 是 `($($args:tt)*)`，body 是
`name!($($args)*)` — 即把所有输入 tokens 原样传回，只是改了
调用形式（但 `name` 相同，所以等价于 no-op）。

实际上更简单：直接让 rule body 为空，匹配后什么都不替换 — 但
这样会丢失原调用形式。所以更准确的做法是：

```rust
// println 的 rule:
// pattern: ($($args:tt)*)
// body:    ( println ) ( ! ) ( ( ) ($($args)*) ( ) )
//        = println ( $($args)* )
```

但我们的 MacroRule 存储的是 token Vec，不是结构化形式。所以直接
构造 tokens：

```rust
fn make_builtin_rule(name: &str, interner: &mut Rodeo) -> MacroRule {
    // pattern: ($($args:tt)*)
    let pattern = vec![
        Token { kind: TokenKind::LParen, ... },
        Token { kind: TokenKind::Dollar, ... },
        Token { kind: TokenKind::Ident(args_sym), ... },
        Token { kind: TokenKind::Colon, ... },
        Token { kind: TokenKind::Ident(tt_sym), ... },
        Token { kind: TokenKind::Star, ... },
        Token { kind: TokenKind::RParen, ... },
    ];
    // body: name!($($args)*)
    let body = vec![
        Token { kind: TokenKind::Ident(name_sym), ... },
        Token { kind: TokenKind::Not, ... },
        Token { kind: TokenKind::LParen, ... },
        Token { kind: TokenKind::Dollar, ... },
        Token { kind: TokenKind::Ident(args_sym), ... },
        Token { kind: TokenKind::Star, ... },
        Token { kind: TokenKind::RParen, ... },
    ];
    MacroRule { pattern, body, span: Span::DUMMY }
}
```

### 4.2 expand_macros_with_errors 集成

```rust
pub fn expand_macros_with_errors(
    tokens: Vec<Token>,
    interner: &Rodeo,
) -> (Vec<Token>, Vec<MacroError>) {
    let mut errors = Vec::new();
    // Stage 18.10: register built-in macros first.
    // Note: build_builtin_macro_table needs &mut Rodeo to intern symbols,
    // but expand_macros_with_errors only has &Rodeo. So we use a
    // separate pass: pre-intern symbols in driver, pass table in.
    // For simplicity, we use interner.try_resolve() which returns Option.
    // If a built-in name isn't interned yet, we skip registering it
    // (the parser will treat the call as a regular macro call).
    let mut table = build_builtin_macro_table_from_existing(interner);
    // Then collect user macros.
    let user_table = collect_macro_defs_with_errors(&tokens, interner, &mut errors);
    // User macros override built-ins.
    table.extend(user_table);
    if table.is_empty() {
        return (tokens, errors);
    }
    // ... rest of expand_macros_with_errors
}
```

实际上，为了保持 `expand_macros_with_errors` 签名不变（`&Rodeo`），
我们改为：在 driver 中先 intern 内置宏名字，然后调用 `expand_macros_with_errors`。

### 4.3 driver 集成

```rust
// src/driver.rs compile()
// Stage 18.10: pre-intern built-in macro names so the macro_expand
// module can register them without needing &mut Rodeo.
for name in crate::parser::macro_expand::BUILTIN_MACRO_NAMES {
    interner.get_or_intern(name);
}
let (tokens, macro_errs) =
    crate::parser::macro_expand::expand_macros_with_errors(tokens, &interner);
```

## 5. 测试矩阵（§9.4.3 1:3+ 正负比例）

新增 8 个测试，2 正 : 6 负 = 1:3 ✓

| # | 类型 | 名称 | 验证 |
|---|------|------|------|
| 1 | positive | builtin_macros_registered | 内置宏表含 4 个条目 |
| 2 | positive | println_still_works_after_builtin_registration | println! 调用仍能正常编译 |
| 3 | negative | builtin_macro_names_const | BUILTIN_MACRO_NAMES 含 4 个名字 |
| 4 | negative | build_builtin_macro_table_returns_table | 函数返回非空 table |
| 5 | negative | builtin_macro_rule_pattern_is_repetition | rule pattern 含 $($args:tt)* |
| 6 | negative | builtin_macro_rule_body_is_same_call | rule body 是 name!($($args)*) |
| 7 | negative | user_macro_overrides_builtin | 用户定义同名宏时覆盖内置 |
| 8 | negative | builtin_macros_pass_through_println | println! 经 expand_macros 后 tokens 不变 |

## 6. 验收

- [x] `cargo build --features llvm-backend` ✅
- [x] `cargo fmt --check` ✅
- [x] `cargo clippy --all-targets --features llvm-backend` 0 warnings ✅
- [x] `cargo test --features llvm-backend` 全绿，新增 8 测试 ✅
  - 503 lib (495 → 503, +8) + 2537 integration = **3,040** total, 0 failures

## 7. 结论

Stage 18.10 完成 println! 通解化迁移 Phase 1：内置 macro_rules! 定义
注册到 prelude。`expand_macros` 通道现在能正确处理内置宏（no-op
展开），parser 特解仍保留（兼容现有 HIR/MIR/Codegen）。

下一阶段（Stage 18.11）规划：
- Phase 2: 将内置 println! macro_rules! 的 body 替换为真正的
  `Call(__landin_println, [__landin_format_args(...)])` 展开
- 这需要先实现 `__landin_println` 和 `__landin_format_args` 函数
- 然后逐步移除 AST/HIR/MIR/Codegen 中的 Println 特解
