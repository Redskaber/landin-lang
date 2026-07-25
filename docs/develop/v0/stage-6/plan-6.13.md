# Stage 6.13 开发计划：lexer/reader.rs 架构性拆分 — 按词法类别 5 模块

> **阶段**: Stage 6.13
> **版本**: v0.13.1 → v0.13.2
> **状态**: 🟡 In Progress
> **流程**: stage-committee-process.md v3.21 §13.4（阶段开始设计对齐）+ §14.4（重构即架构设计）

## 1. 阶段开始设计对齐（§13.4 强制）

### 1.1 对应设计文档

| 设计文档 | 章节 | 用途 |
|---------|------|------|
| `docs/lang-design/02-grammar.md` | §1.1-§1.9 词法结构 | Lexer 设计基线 |
| `docs/lang-design/05-ast.md` | 全文 | Token → AST 数据流 |

### 1.2 设计意图摘要（02-grammar.md §1）

设计文档 §1 把词法结构分为 9 个子节：
- §1.1 字符集（UTF-8 / 空白 / 注释 / BOM）
- §1.2 Token 分类（keyword / identifier / literal / operator / punctuation / whitespace / comment）
- §1.3 关键字（严格保留 + 弱保留 + MVP 保留）
- §1.4 Identifier（XID_Start + XID_Continue + raw identifier `r#`）
- §1.5 整数字面量（dec / hex / oct / bin + integer_suffix）
- §1.6 浮点字面量（float_lit + float_suffix）
- §1.7 字符与字符串（char / byte / string / raw_string / byte_string / raw_byte_string + escape）
- §1.8 运算符与标点（operator + punctuation）
- §1.9 长度规则（Maximal Munch）

### 1.3 当前实现 vs 设计文档

#### 已对齐项

- ✅ Maximal Munch 原则（设计 §1.9）
- ✅ 所有 9 个子节对应 token 类型都已实现
- ✅ 嵌套块注释（设计 §1.1）
- ✅ raw identifier `r#`（设计 §1.4）
- ✅ 4 种整数字面量 + 后缀（设计 §1.5）
- ✅ 浮点字面量 + 后缀（设计 §1.6）
- ✅ char / byte / string / raw_string / byte_string / raw_byte_string + escape（设计 §1.7）
- ✅ 所有 operator + punctuation（设计 §1.8）

#### 已知偏差

- **B3 实现 ≠ 设计（结构层面）**：设计文档 §1 把词法结构分为 9 个子节，
  但实现把 40+ lex 函数堆在单一 `reader.rs`（1537 LOC），违反 §14.4 J2
  （单一职责）和 J6（科学合理粒度）。

### 1.4 本阶段灰区决策

| 灰区 | 决策 | 理由 |
|------|------|------|
| 拆分粒度？ | 按设计 §1 的子节聚合为 5 个子模块 | 与设计文档对齐（§14.4 J1） |
| 是否拆分 LexError？ | 不拆，保留在 reader.rs | 与 Lexer struct 紧密耦合 |
| 是否拆分 cursor 操作？ | 不拆，保留在 reader.rs | 单一职责边界（cursor = 输入游标） |
| `next_token` 调度器？ | 保留在 reader.rs | 是入口点，与 cursor + trivia 紧密耦合 |
| `pub` 可见性？ | 所有 lex_* 函数 `pub(super)` | §16 隔离——lexer 外部只应看到 `next_token` 入口 |

## 2. §14.4 J1-J6 判据检查

### 2.1 J1 架构设计对齐 ✅

新结构按 02-grammar.md §1 划分（聚合 9 子节为 5 个内聚模块）：

| 设计文档章节 | 新模块 | 内容 |
|------------|--------|------|
| §1.3 关键字 + §1.4 Identifier | `ident.rs` | lex_ident + lex_raw_identifier + is_ident_start_byte |
| §1.5 + §1.6 数值字面量 | `number.rs` | lex_number + lex_hex + lex_oct + lex_bin + try_lex_number_suffix |
| §1.7 字符与字符串 | `string.rs` | lex_string + lex_raw_string + lex_raw_string_hash + lex_byte_string + lex_byte + lex_byte_escape + lex_raw_byte_string + lex_char_or_lifetime + lex_escape + lex_escape_from_str |
| §1.1 注释 + §1.8 运算符 | `operators.rs` | lex_doc_comment + 14 个 lex_<op> 函数（dot/lt/gt/eq/bang/plus/minus/star/slash/percent/and/or/caret/colon） |
| §1.2 + cursor + 入口 | `reader.rs` | Lexer struct + cursor + skip_trivia + next_token + LexError |

### 2.2 J2 单一职责 ✅

每个新模块承担且仅承担一个明确的职责：
- `ident.rs` = "解析 identifier + raw identifier + 关键字识别"
- `number.rs` = "解析整数（dec/hex/oct/bin）+ 浮点 + 后缀"
- `string.rs` = "解析字符与字符串字面量（6 种）+ escape"
- `operators.rs` = "解析注释 + 运算符 + 标点"
- `reader.rs` = "Lexer 入口 + cursor + trivia skip + next_token 调度"

### 2.3 J3 单向流动 ✅

模块依赖图：

```
reader.rs (cursor + next_token dispatcher)
  ↓ 调用
ident.rs / number.rs / string.rs / operators.rs (各 lex_* 实现)
```

无反向依赖：子模块不调用 reader.rs 的 next_token/skip_trivia。
无循环依赖：所有子模块只通过 `Lexer` 的方法调用 + cursor 操作。

### 2.4 J4 编译相关表达完整 ✅

每个模块的"编译相关概念"在模块内是完整的：
- `ident.rs`：identifier 词法 + 关键字表内聚
- `number.rs`：所有数值字面量 + 后缀识别内聚
- `string.rs`：所有字符/字符串 + escape 规则内聚
- `operators.rs`：所有运算符 + 标点 + 注释内聚

### 2.5 J5 阶段划分清晰 ✅

所有新模块仍在 `src/lexer/` 目录下，仍是 Stage 0 阶段。不破坏 §16 阶段隔离。

### 2.6 J6 科学合理粒度 ✅

拆分后 LOC 分布（估算）：

| 模块 | 估算 LOC | 设计依据 |
|------|---------|---------|
| `reader.rs` | ~330 | Lexer struct + cursor + skip_trivia + next_token + LexError |
| `ident.rs` | ~120 | lex_ident + lex_raw_identifier + is_ident_start_byte |
| `number.rs` | ~340 | lex_number + lex_hex/oct/bin + try_lex_number_suffix |
| `string.rs` | ~580 | 10 个字符串/字符函数 |
| `operators.rs` | ~280 | lex_doc_comment + 14 个 lex_<op> |
| **总计** | ~1650 | （含模块头注释略增） |

每个模块均在 100-1500 LOC 合理区间，reader.rs 远低于 1500 阈值。

## 3. 拆分方案

### 3.1 目标组织结构

```
src/lexer/
  mod.rs          (50 LOC, 不变)  — crate-level re-exports
  reader.rs       (~330 LOC, -78%) ← Lexer struct + cursor + skip_trivia + next_token + LexError
  ident.rs        (新, ~120 LOC)  ← identifier + raw identifier + 关键字（§1.3+§1.4）
  number.rs       (新, ~340 LOC)  ← 数值字面量（§1.5+§1.6）
  string.rs       (新, ~580 LOC)  ← 字符与字符串（§1.7）
  operators.rs    (新, ~280 LOC)  ← 注释 + 运算符 + 标点（§1.1+§1.8）
  token.rs        (390 LOC, 不变) — Token 类型定义
```

### 3.2 可见性策略（与 Stage 6.12 parser 一致）

- `Lexer` struct 字段全部 `pub(super)`
- 所有 cursor 方法（`peek`/`peek_at`/`bump`/`span_from`）`pub(super)`
- `skip_trivia` `pub(super)`（next_token 调用）
- 所有 `lex_*` 方法 `pub(super)`
- `next_token` 保持 `pub`（外部入口，driver 调用）
- `into_errors` / `is_at_end` 保持 `pub`
- `is_ident_start_byte` `pub(super)`

### 3.3 §23 API 命名合规

- 所有函数名保留原名（零 churn）
- 模块名遵循 `<noun>` 模式（与既有 `token.rs`、`error.rs` 风格一致）
- 无新公共符号（纯架构性重组）
- `next_token` 仍是唯一 `pub` 入口

### 3.4 §16 接口隔离合规

- 子模块通过 `impl Lexer` 方法访问 cursor，不直接读字段
- 数据流单向：reader.rs 入口 → next_token 分发 → 各 lex_* 子模块 → Token
- 无跨阶段调用

## 4. 风险与缓解

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| 字段可见性不足 | 中 | 编译失败 | 把 cursor 方法和字段改 `pub(super)` |
| impl 跨文件导致方法找不到 | 低 | 编译失败 | 每个子模块独立 `impl<'a> Lexer<'a> { ... }` |
| 移动函数时遗漏共享辅助 | 中 | 编译失败 | 提取共享 helper 到 reader.rs（pub(super)） |
| 1881 测试回归 | 低 | 测试失败 | 行为等价拆分，逐模块迁移 + cargo test 验证 |

## 5. 验收标准（§1.2）

- [ ] `cargo clean && cargo test` — 1881 tests 全过
- [ ] `cargo fmt` — clean
- [ ] `cargo clippy --all-targets` — 0 warnings, 0 errors
- [ ] `lexer/reader.rs` 降到 ~330 LOC（-78%）
- [ ] 4 个新子模块各自单一职责
- [ ] 文档：plan-6.13.md + gate-review-6.13.md + dev-log + api-naming-standard v1.82 + RELEASE_NOTES + README + worklog
- [ ] 版本 v0.13.1 → v0.13.2

## 6. 后续 Stage 6.14+ 候选

完成本轮后：

- **Stage 6.14**: borrowck/mod.rs 拆分（1452 LOC → 按分析类别）
- **Stage 6.15**: typeck/checker.rs 拆分（1320 LOC → 按检查类别）
- **Stage 6 末尾**: 完整 §25.8 设计回写（全 docs/lang-design/）
- **TD-015**: Region inference
- **TD-018**: 用户自定义 trait dyn 支持

---

**创建日期**: 2026-07-25
