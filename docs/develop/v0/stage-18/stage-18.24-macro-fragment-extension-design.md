# Stage 18.24 — Macro Fragment Specifier Extension (lifetime + stmt)

> **Author**: redskaber + ARCH-A + REV-A
> **Date**: 2026-08-06
> **Version**: v0.304.0
> **Process**: stage-committee-process.md v5.0 §13.5 (1 轮自审定稿)
> **Status**: ✅ Complete

## 1. 阶段目标

**背景**: Stage 18.05 添加了 4 个 fragment specifiers (ty/literal/block/path)，
总计 7 个。本阶段添加 2 个新的 fragment specifiers，总计 9 个。

**具体目标**:
1. `$name:lifetime` — 匹配一个生命周期标识符 (`'a`, `'static`, etc.)
2. `$name:stmt` — 匹配单个语句 (tokens until `;` or `}`)

## 2. 设计原则

| 原则 | 落实方式 |
|------|----------|
| §1.0 原則 6 "通用 > 特解" | 一个 match frag 调度所有 fragments |
| §10 命名 | `capture_lifetime` / `capture_stmt` |
| §11 接口隔离 | capture 函数在 macro_expand.rs 内部 |
| 单一职责 | 每个 capture_* 只处理一种 fragment |
| 高内聚低耦合 | fragment 调度集中在 match_pattern |
| 避免死代码 | 所有新函数被 match_pattern 调用 |
| 避免分散内容 | fragment 逻辑集中 |

## 3. 实现

### 3.1 capture_lifetime

```rust
/// Capture a lifetime: a single Lifetime token.
fn capture_lifetime(input: &[Token], idx: &mut usize) -> Vec<Token> {
    if *idx < input.len() {
        if let TokenKind::Lifetime(_) = &input[*idx].kind {
            let token = input[*idx].clone();
            *idx += 1;
            return vec![token];
        }
    }
    Vec::new()
}
```

### 3.2 capture_stmt

```rust
/// Capture a statement: tokens until top-level `;` or `}`.
fn capture_stmt(input: &[Token], idx: &mut usize) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut depth = 0i32;
    while *idx < input.len() {
        match &input[*idx].kind {
            TokenKind::LParen | TokenKind::LBracket | TokenKind::LBrace => {
                depth += 1;
                tokens.push(input[*idx].clone());
            }
            TokenKind::RParen | TokenKind::RBracket | TokenKind::RBrace => {
                if depth == 0 { break; }
                depth -= 1;
                tokens.push(input[*idx].clone());
            }
            TokenKind::Semicolon if depth == 0 => {
                tokens.push(input[*idx].clone());
                *idx += 1;
                break;
            }
            _ => tokens.push(input[*idx].clone()),
        }
        *idx += 1;
    }
    tokens
}
```

### 3.3 match_pattern 扩展

```rust
let captured = match frag {
    "expr" => capture_expr(input, &mut ii),
    "ident" => capture_ident(input, &mut ii),
    "tt" => capture_tt(input, &mut ii),
    "ty" => capture_ty(input, &mut ii),
    "literal" => capture_literal(input, &mut ii),
    "block" => capture_block(input, &mut ii),
    "path" => capture_path(input, &mut ii),
    "lifetime" => capture_lifetime(input, &mut ii),  // Stage 18.24
    "stmt" => capture_stmt(input, &mut ii),           // Stage 18.24
    _ => return false,
};
```

## 4. 测试矩阵（§9.4.3 1:3+ 正负比例）

新增 8 个测试，2 正 : 6 负 = 1:3 ✓

| # | 类型 | 名称 | 验证 |
|---|------|------|------|
| 1 | positive | macro_with_lifetime_fragment | `$l:lifetime` 解析通过 |
| 2 | positive | macro_with_stmt_fragment | `$s:stmt` 解析通过 |
| 3 | negative | capture_lifetime_simple | capture_lifetime 收集 `'a` |
| 4 | negative | capture_lifetime_rejects_non_lifetime | 非 lifetime → empty |
| 5 | negative | capture_stmt_until_semicolon | capture_stmt 收集到 `;` |
| 6 | negative | capture_stmt_until_rbrace | capture_stmt 收集到 `}` |
| 7 | negative | capture_stmt_nested_braces | 嵌套大括号正确处理 |
| 8 | negative | lifetime_fragment_in_pattern | pattern 中 lifetime fragment 匹配 |

## 5. 验收

- [x] `cargo build --features llvm-backend` ✅
- [x] `cargo fmt --check` ✅
- [x] `cargo clippy --all-targets --features llvm-backend` 0 warnings ✅
- [x] `cargo test --features llvm-backend` 全绿，新增 8 测试 ✅
  - 583 lib (575 → 583, +8) + 2537 integration = **3,120** total, 0 failures
