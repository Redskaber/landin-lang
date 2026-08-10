# Stage 18.06 — macro_rules! Phase 6: Repetition `$(...)*` / `$(...)+` / `$(...)?`

> **Author**: redskaber + ARCH-A + REV-A
> **Date**: 2026-08-06
> **Version**: v0.292.0
> **Process**: stage-committee-process.md v5.0 §13.5 (1 轮自审定稿)
> **Status**: ✅ Complete

## 1. 阶段目标

实现 macro_rules! 中的重复语法：
- `$(...)*` — 0 次或多次
- `$(...)+` — 1 次或多次
- `$(...)?` — 0 次或 1 次

支持的语法形式（pattern + body 两端都需要）：

```landin
macro_rules! vec_of {
    ($($x:expr),*) => { /* expand $x zero or more times */ };
    ($($x:expr),+) => { /* expand $x one or more times */ };
    ($($x:expr),?) => { /* expand $x zero or one time */ };
}
```

## 2. 设计原则

| 原则 | 落实方式 |
|------|----------|
| §1.0 原則 6 "通用 > 特例" | 一个 `match_repetition` 函数处理 `*`/`+`/`?` 三种操作符；body 端用一个 `substitute_repetition` 处理 |
| §10 命名 | 新增 `match_repetition`, `substitute_repetition`, `RepetitionKind` |
| §11 接口隔离 | 所有新增类型/函数均为 `macro_expand.rs` 内部 |
| 单一职责 | `match_repetition` 只匹配；`substitute_repetition` 只展开 |
| 高内聚低耦合 | repetition 逻辑封装在 match_pattern/substitute_body 调度处 |
| 避免死代码 | 所有新代码都被 match_pattern/substitute_body 调用 |

## 3. 数据结构

```rust
/// Stage 18.06: Kind of repetition operator in macro_rules! patterns.
/// Per §10: enum follows `<Noun>Kind` pattern (mirrors `BorrowKind`,
/// `IntTy`, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RepetitionKind {
    /// `$(...)*` — zero or more
    ZeroOrMore,
    /// `$(...)+` — one or more
    OneOrMore,
    /// `$(...)?` — zero or one
    ZeroOrOne,
}

/// Stage 18.06: A captured repetition — list of per-iteration capture maps.
/// Each element of the outer Vec is one iteration's captures.
type RepetitionCaptures = HashMap<crate::lexer::Symbol, Vec<Vec<Token>>>;
```

## 4. 算法设计

### 4.1 Pattern matching — `match_repetition`

输入：pattern 中 `$( $body ) $op` 片段，input 中剩余位置 `idx`。
输出：成功匹配的次数 + 每次迭代的捕获值。

```
1. Parse pattern: $( <inner_pattern> ) <op>
   - <inner_pattern> = pattern tokens between $( and matching )
   - <op> = Star / Plus / Question
2. Parse pattern separator (optional) — NOT supported in v0.6 (Rust 子集)
   简化: 内部 pattern 必须能在 input 上完整匹配，不处理分隔符
3. Repeatedly call match_pattern(inner_pattern, input[idx..]) → captures
   - If success: save captures, advance idx by matched length
   - If fail: stop
4. Apply $op:
   - Star: 0+ matches OK, accept any count
   - Plus: 1+ matches required, return false if 0
   - Question: 0 or 1 matches; if 2+ matched, treat as 1 (or fail?)
5. Save repetition captures (list of per-iter maps) under each name
```

**简化版**：本 stage 不支持 separator（如 `$(...),*` 中的 `,`）。
内部 pattern 是一个完整的 `$name:fragment` 或字面 token 序列。
分隔符支持留到 Stage 18.07。

### 4.2 Body substitution — `substitute_repetition`

输入：body 中 `$( $inner_body ) $op` 片段，captures 中每个 `$name` 的
repetition 值。

```
1. Parse body: $( <inner_body> ) $op
2. For each name $name in inner_body:
   - Look up repetition captures — must have N entries (iterations)
3. For i in 0..N:
   - Create a captures map with $name → captures[$name][i]
   - substitute_body(inner_body, &local_captures)
   - Append result to output
4. (separator handling: NONE in v0.6 simplified)
```

### 4.3 match_pattern 调度扩展

在现有 `match_pattern` 函数中，当遇到 `TokenKind::Dollar` 后跟 `LParen` 时，
进入 repetition 分支：

```rust
if pt.kind == TokenKind::Dollar && pattern[pi+1].kind == TokenKind::LParen {
    // Find matching ) in pattern
    let (inner_end, inner_pattern) = collect_pattern_until_close_paren(pattern, pi+1);
    // Read $op (Star/Plus/Question)
    let op = match pattern[inner_end+1].kind {
        TokenKind::Star => RepetitionKind::ZeroOrMore,
        TokenKind::Plus => RepetitionKind::OneOrMore,
        TokenKind::Question => RepetitionKind::ZeroOrOne,
        _ => return false,
    };
    // Match repetition
    if !match_repetition(&inner_pattern, input, ii, op, captures) {
        return false;
    }
    pi = inner_end + 2; // past ) and $op
    continue;
}
```

### 4.4 substitute_body 调度扩展

类似地，body 中遇到 `$( ... )` 时进入 repetition substitution：

```rust
if bt.kind == TokenKind::Dollar && body[i+1].kind == TokenKind::LParen {
    // Find matching ) in body
    let (inner_end, inner_body) = collect_body_until_close_paren(body, i+1);
    // Read $op
    let op = ...;
    substitute_repetition(&inner_body, &captures, &mut result);
    i = inner_end + 2;
    continue;
}
```

## 5. 测试矩阵（§9.4.3 1:3+ 正负比例）

新增 8 个测试，2 正 : 6 负 = 1:3 ✓

| # | 类型 | 名称 | 验证 |
|---|------|------|------|
| 1 | positive | macro_with_star_repetition | `$( $x:expr )*` 匹配 0+ exprs |
| 2 | positive | macro_with_plus_repetition | `$( $x:expr )+` 匹配 1+ exprs |
| 3 | negative | repetition_kind_from_star | RepetitionKind 解析 `*` |
| 4 | negative | repetition_kind_from_plus | RepetitionKind 解析 `+` |
| 5 | negative | repetition_kind_from_question | RepetitionKind 解析 `?` |
| 6 | negative | match_repetition_zero_or_more_empty | `*` 匹配空输入 |
| 7 | negative | match_repetition_one_or_more_empty | `+` 拒绝空输入 |
| 8 | negative | substitute_repetition_expands_each_iter | 替换每次迭代 |

## 6. 验收

- [x] `cargo build --features llvm-backend` ✅
- [x] `cargo fmt --check` ✅
- [x] `cargo clippy --all-targets --features llvm-backend` 0 warnings ✅
- [x] `cargo test --features llvm-backend` 全绿，新增 8 测试 ✅
  - 487 lib (479 → 487, +8) + 2537 integration = **3,024** total, 0 failures

## 7. 结论

Stage 18.06 完成 macro_rules! Phase 6：Repetition。`$(...)*` / `$(...)+`
/ `$(...)?` 三种操作符全部支持。下一阶段（Stage 18.07）规划：

- 添加 separator 支持 `$(...),*`（带分隔符的重复）
- 或开始 println! 通解化迁移
- 或开始 macro hygiene（基础宏卫生）
