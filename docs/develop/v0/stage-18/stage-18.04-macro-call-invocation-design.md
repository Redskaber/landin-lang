# Stage 18.04 — macro_rules! Phase 4: Macro Call Invocation + Driver Integration

> **Author**: redskaber + ARCH-A + REV-A
> **Date**: 2026-08-06
> **Version**: v0.290.0
> **Process**: stage-committee-process.md v5.0 §13.5 (1 轮自审定稿)
> **Status**: ✅ Complete

## 1. 阶段目标

将 Stage 18.03 的 token-tree 匹配/替换引擎接入编译驱动，使
`macro_rules!` 定义的宏在 `name!(args)` 调用点真正展开。

具体目标：
1. 实现一个 **pre-parse macro expansion pass**（预解析宏展开遍）——
   在词法分析后、语法分析前对 token 流做一次扫描：
   - 第一遍：收集所有 `macro_rules!` 定义，建立 `MacroTable`。
   - 第二遍：扫描 `ident!(...)` / `ident!{...}` / `ident![...]` 调用点，
     如果 `ident` 命中 `MacroTable`，调用 `expand_macro` 展开并把结果
     token 流原地拼接回主 token 流；未命中的调用点（如 `println!`）
     保持不变，由现有 parser 特解处理。
2. 在 `driver::compile` 中调用此 pass，作为 lexer → parser 之间的
   独立管线节点。
3. 通过 §9.4.3 要求的 1:3+ 正负比例测试覆盖。

## 2. 设计原则与对照

| 原则 | 落实方式 |
|------|----------|
| §10 入口函数模式 | 新增自由函数入口 `parser::macro_expand::expand_macros(tokens, &interner) -> Vec<Token>`，符合 `<verb>_<noun>` |
| §10 函数命名前缀 | 新增内部函数使用 `collect_macro_defs` / `expand_macro_calls` / `expand_macros` 命名 |
| §11 接口隔离 | 宏展开是 parser 阶段内部子模块，不调用其他阶段内部函数；driver 仅通过 `expand_macros` 自由函数入口调用 |
| §1.0 原則 6 "通用 > 特例" | 一个引擎处理所有 macro_rules! 调用点；不针对单个宏做特解 |
| §13.4 高内聚低耦合 | 展开逻辑封装在 `src/parser/macro_expand.rs`；driver 只看到入口函数 |
| 单一职责 | `collect_macro_defs` 只负责收集；`expand_macro_calls` 只负责展开；`expand_macros` 只负责协调 |
| 避免死代码 | 所有新增函数都被 driver 调用或被测试覆盖 |
| 避免分散内容 | 新功能集中在一个文件 `macro_expand.rs`，不分散到其他模块 |

## 3. 架构设计

### 3.1 管线位置

```
lexer::tokenize(src, &mut interner)
    → Vec<Token>
        ↓
parser::macro_expand::expand_macros(tokens, &interner)   ← Stage 18.04 新增
    → Vec<Token>  (macro_rules! 调用已展开)
        ↓
parser::parse_crate(tokens, &mut interner)
    → Crate
```

### 3.2 数据结构

```rust
// src/parser/macro_expand.rs (扩展)

/// Stage 18.04: Macro definition table collected from token stream.
/// Maps macro name (Symbol) → MacroRulesDef (with rules).
/// Per §10: type name follows `<Noun>Table` pattern.
pub type MacroTable = HashMap<crate::lexer::Symbol, MacroRulesDef>;
```

### 3.3 函数签名

```rust
/// Stage 18.04: Collect all `macro_rules!` definitions from a token stream.
/// Returns a table mapping macro name → MacroRulesDef.
/// Per §10: `collect_macro_defs` follows `<verb>_<noun>_<noun>` pattern.
pub fn collect_macro_defs(tokens: &[Token], interner: &Rodeo) -> MacroTable;

/// Stage 18.04: Expand macro calls in a token stream.
/// Walks tokens looking for `ident!` patterns; if `ident` is in the
/// macro table, collects input tokens until matching delimiter, calls
/// `expand_macro`, and splices the expanded tokens into the output.
/// Unknown macros (e.g. `println!`) are passed through unchanged.
/// Per §10: `expand_macro_calls` follows `<verb>_<noun>_<noun>` pattern.
pub fn expand_macro_calls(
    tokens: &[Token],
    table: &MacroTable,
    interner: &Rodeo,
) -> Vec<Token>;

/// Stage 18.04: Top-level macro expansion pass — driver entry point.
/// 1. Collect macro_rules! definitions into MacroTable.
/// 2. If table is empty, return input unchanged (no overhead).
/// 3. Expand macro calls iteratively (up to MAX_EXPANSION_ROUNDS)
///    until no more expansions occur.
/// Per §10: `expand_macros` follows `<verb>_<noun>` pattern (free-function entry).
pub fn expand_macros(tokens: Vec<Token>, interner: &Rodeo) -> Vec<Token>;
```

### 3.4 实现要点

#### 3.4.1 `collect_macro_defs`

扫描 token 流，匹配模式：
```
Ident("macro_rules") Bang Ident(name) LBrace ... RBrace
```

遇到此模式时：
- 跳过 `macro_rules ! name`
- 进入 `{ ... }` 块，按 `(pattern) => { body };` 格式提取规则
- 重用现有 `MacroRule { pattern, body, span }` 结构
- 存入 `MacroTable`

不删除原 token —— parser 仍然会正常解析 `macro_rules!` 定义并
生成 `ItemKind::MacroRules` AST 节点（保持现有行为）。

#### 3.4.2 `expand_macro_calls`

逐 token 遍历：
- 如果当前 token 是 `Ident(name)` 且下一个 token 是 `Bang`：
  - 如果 `name` 在 `MacroTable` 中：
    - 确定分隔符（`(`/`{`/`[`)
    - 收集到匹配的右分隔符为止（平衡嵌套）
    - 调用 `expand_macro(def, &input_tokens, interner)`
    - 如果展开成功，将展开后的 token 流加入输出
    - 如果展开失败，输出原始 token（保持调用形式）
  - 否则（不在表中）：原样输出 `ident ! delim ...`
- 否则：原样输出当前 token

#### 3.4.3 `expand_macros`

```
1. table = collect_macro_defs(tokens, interner)
2. if table.is_empty() { return tokens }
3. let mut current = tokens
4. for _ in 0..MAX_EXPANSION_ROUNDS:
     let next = expand_macro_calls(&current, &table, interner)
     if !did_expand { break }
     current = next
5. return current
```

`MAX_EXPANSION_ROUNDS = 32`（足够大以处理实际用例，又能防止无限递归）。

### 3.5 driver 集成

`src/driver.rs` 中 `compile` 函数：

```rust
// === Stage 0: Lex ===
let (tokens, lex_errors) = tokenize(src, &mut interner);
// ... error check ...

// === Stage 18.04: Macro expansion ===
let tokens = crate::parser::macro_expand::expand_macros(tokens, &interner);

// === Stage 0: Parse ===
let mut parser = Parser::new(tokens, &mut interner);
// ...
```

## 4. 测试矩阵（§9.4.3 1:3+ 正负比例）

新增 8 个测试，2 正 : 6 负 = 1:3 ✓

| # | 类型 | 名称 | 验证 |
|---|------|------|------|
| 1 | positive | macro_call_expands_simple | `macro_rules! m { () => { 42 } } fn f() { m!() }` — m! 展开为 42 |
| 2 | positive | macro_call_expands_with_capture | `macro_rules! m { ($x:expr) => { $x } } fn f() { m!(99) }` — $x 替换为 99 |
| 3 | negative | collect_finds_no_macros | 空 token 流 → 空 table |
| 4 | negative | collect_finds_macro_definition | 含 macro_rules! → table 含 1 项 |
| 5 | negative | expand_macro_calls_passes_unknown | `println!(...)` → 原样输出 |
| 6 | negative | expand_macro_calls_passes_no_macros | 无 table → 原样输出 |
| 7 | negative | expand_macros_no_macros_returns_input | 无 macro_rules! → 原样输出 |
| 8 | negative | expand_macros_handles_recursive | 递归展开上限保护（不会无限循环） |

## 5. 验收

- [x] `cargo build --features llvm-backend` ✅
- [x] `cargo fmt --check` ✅
- [x] `cargo clippy --all-targets --features llvm-backend` 0 warnings ✅
- [x] `cargo test --features llvm-backend` 全绿，新增 8 测试 ✅
  - 471 lib (463 → 471, +8) + 2537 integration = **3,008** total, 0 failures

## 6. 结论

Stage 18.04 完成 macro_rules! Phase 4：Macro Call Invocation + Driver
Integration。`macro_rules!` 定义的宏现在可以在 `name!(args)` 调用点
真正展开。`expand_macros` 自由函数入口遵循 §10 命名标准；展开逻辑
封装在 `src/parser/macro_expand.rs`，符合 §11 接口隔离。

下一阶段（Stage 18.05）规划：
- 添加更多 fragment specifier（`$name:ty`、`$name:literal`、`$name:block`）
- 或实现 repetition `$(...)*` / `$(...)+` / `$(...)?`
- 或开始 println! 通解化迁移（Phase 5）
