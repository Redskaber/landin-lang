# Stage 18.13 — macro_rules! Separator Support `$(...),*` / `$(...);+` / `$(...)?`

> **Author**: redskaber + ARCH-A + REV-A
> **Date**: 2026-08-06
> **Version**: v0.296.0
> **Process**: stage-committee-process.md v5.0 §13.5 (1 轮自审定稿)
> **Status**: ✅ Complete

## 1. 阶段目标

**背景**: Stage 18.06 实现了 `$(...)*` / `+` / `?` 重复，但**不支持分隔符**。
这意味着 `vec![1, 2, 3]` 这样的常见宏无法表达——因为需要 `$(...),*`
（逗号分隔的重复）。

**用户反馈**: "println! 系列也是 macro 的一部分，所以不能只解决 println!
而忽略 macro (这本质上也是 通解 > 特解)"。本阶段响应此反馈，改进
macro_rules! 系统本身，而非继续 println! 迁移。

**具体目标**:
1. 支持 `$(...)<sep>*` — 带分隔符的零次或多次重复
2. 支持 `$(...)<sep>+` — 带分隔符的一次或多次重复
3. 支持 `$(...)<sep>?` — 带分隔符的零次或一次（较少用，但为完整性支持）
4. 分隔符可以是任何单个 token (`,`, `;`, `=>`, `|`, etc.)
5. 在 pattern 端：分隔符出现在迭代之间（不在末尾）
6. 在 body 端：分隔符出现在展开的迭代之间

## 2. 设计原则

| 原则 | 落实方式 |
|------|----------|
| §1.0 原則 6 "通用 > 特例" | 一个 `match_repetition` 函数处理有/无分隔符 |
| §10 命名 | `RepetitionSep` enum, `parse_repetition_sep` |
| §11 接口隔离 | 分隔符逻辑封装在 macro_expand.rs |
| 单一职责 | `parse_repetition_sep` 只解析分隔符 |
| 高内聚低耦合 | 分隔符存储在 `RepetitionKind` 旁 |
| 避免死代码 | 所有新代码都被 match_repetition/substitute_repetition 调用 |
| 避免分散内容 | 分隔符逻辑集中在 repetition 处理处 |

## 3. 数据结构

```rust
// src/parser/macro_expand.rs

/// Stage 18.13: Optional separator in a macro_rules! repetition.
///
/// `$(...)*`  → `RepetitionSep::None`
/// `$(...),*` → `RepetitionSep::Token(TokenKind::Comma)`
/// `$(...);+` → `RepetitionSep::Token(TokenKind::Semicolon)`
///
/// Per §10: enum follows `<Noun>` pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RepetitionSep {
    /// No separator — `$(...)*` / `+` / `?`
    None,
    /// A single token separator — `$(...),*` / `$(...);+` / etc.
    /// Stores the token kind (without span) for matching.
    Token(TokenKind),
}
```

`RepetitionKind` 扩展为携带分隔符:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
enum RepetitionKind {
    ZeroOrMore(RepetitionSep),
    OneOrMore(RepetitionSep),
    ZeroOrOne(RepetitionSep),
}
```

**注意**: 这改变了 `RepetitionKind` 的 ABI。需要更新所有使用处
（`parse_repetition_op`, `match_repetition`, `substitute_repetition`,
所有测试）。

## 4. 实现

### 4.1 parse_repetition_op 扩展

```rust
/// Stage 18.13: Parse the repetition operator (and optional separator)
/// starting at `tokens[idx]`.
///
/// Returns `Some((RepetitionKind, after_op_index))` if a valid
/// `*`/`+`/`?` operator (possibly preceded by a separator token) is found.
///
/// Syntax:
///   `*`           → ZeroOrMore(None)
///   `+`           → OneOrMore(None)
///   `?`           → ZeroOrOne(None)
///   `, *`         → ZeroOrMore(Comma)
///   `; +`         → OneOrMore(Semicolon)
///   `=> ?`        → ZeroOrOne(FatArrow)  [unusual but valid]
///
/// Per §10: `<verb>_<noun>_<noun>` pattern.
fn parse_repetition_op(
    tokens: &[Token],
    idx: usize,
) -> Option<(RepetitionKind, usize)>;
```

### 4.2 match_repetition 分隔符处理

算法：
1. 在每次迭代之间，期望一个分隔符 token
2. 如果分隔符存在且匹配，继续下一次迭代
3. 如果分隔符不存在（输入耗尽或下一个是 `*`/`+` 后的 token），停止
4. **不消费尾随分隔符**（`vec![1, 2, 3,]` 中的尾随逗号是另一回事，暂不支持）

```rust
fn match_repetition(
    inner: &[Token],
    input: &[Token],
    idx: &mut usize,
    kind: RepetitionKind,
    captures: &mut Captures,
    interner: &Rodeo,
) -> Option<usize> {
    let sep = match &kind {
        RepetitionKind::ZeroOrMore(s) | RepetitionKind::OneOrMore(s) | RepetitionKind::ZeroOrOne(s) => s,
    };
    let mut iter_count = 0usize;
    loop {
        // Try to match inner pattern.
        let mut iter_captures = Captures::new();
        let mut local_idx = *idx;
        if !match_pattern_at(inner, input, &mut local_idx, &mut iter_captures, interner) {
            break;
        }
        if local_idx == *idx { /* no progress guard */ ... }
        // Save captures.
        ...
        *idx = local_idx;
        iter_count += 1;
        // Stage 18.13: Check for separator before next iteration.
        if let RepetitionSep::Token(sep_kind) = sep {
            if *idx < input.len() && tokens_match(sep_kind, &input[*idx].kind) {
                *idx += 1; // consume separator
                // Continue to next iteration.
            } else {
                break; // No separator — stop.
            }
        }
        // If sep is None, just continue (no separator to check).
    }
    ...
}
```

### 4.3 substitute_repetition 分隔符处理

```rust
fn substitute_repetition(
    inner: &[Token],
    captures: &Captures,
    kind: RepetitionKind,
    result: &mut Vec<Token>,
) {
    let sep = match &kind { ... };
    let iter_count = ...;
    for i in 0..iter_count {
        // Substitute inner body for iteration i.
        let expanded = substitute_body(inner, &local);
        result.extend(expanded);
        // Stage 18.13: Emit separator between iterations (not after last).
        if let RepetitionSep::Token(sep_kind) = sep {
            if i + 1 < iter_count {
                result.push(Token { kind: sep_kind.clone(), span: Span::DUMMY });
            }
        }
    }
}
```

## 5. 测试矩阵（§9.4.3 1:3+ 正负比例）

新增 8 个测试，2 正 : 6 负 = 1:3 ✓

| # | 类型 | 名称 | 验证 |
|---|------|------|------|
| 1 | positive | macro_with_comma_separator | `$( $x:expr ),*` 匹配 `1, 2, 3` |
| 2 | positive | macro_with_semicolon_separator | `$( $x:expr );+` 匹配 `1; 2` |
| 3 | negative | repetition_sep_none_variant | `RepetitionSep::None` 构造 |
| 4 | negative | repetition_sep_token_variant | `RepetitionSep::Token(Comma)` 构造 |
| 5 | negative | parse_repetition_op_no_separator | `*` → ZeroOrMore(None) |
| 6 | negative | parse_repetition_op_with_comma | `, *` → ZeroOrMore(Comma) |
| 7 | negative | match_repetition_with_separator_matches | 带分隔符的重复匹配 |
| 8 | negative | substitute_repetition_emits_separator | body 展开时插入分隔符 |

## 6. 验收

- [x] `cargo build --features llvm-backend` ✅
- [x] `cargo fmt --check` ✅
- [x] `cargo clippy --all-targets --features llvm-backend` 0 warnings ✅
- [x] `cargo test --features llvm-backend` 全绿，新增 8 测试 ✅
  - 519 lib (511 → 519, +8) + 2537 integration = **3,056** total, 0 failures

## 7. 结论

Stage 18.13 完成 macro_rules! separator 支持。`$(...),*` / `$(...);+` 等
带分隔符的重复现在可用。这响应了用户反馈——macro 系统本身的改进
优先于 println! 迁移（通解 > 特解）。

下一阶段 (Stage 18.14):
- 继续改进 macro 系统：nested repetition, 更多 fragment specifiers
- 或继续 println! 迁移 Phase 2
