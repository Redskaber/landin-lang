# Stage 6 Gate Review Round 13 (6.13) — lexer/reader.rs architectural split per §14.4

> **审查日期**: 2026-07-25 | **版本**: v0.13.1 → v0.13.2
> **流程**: stage-committee-process.md v3.21 §13.4（阶段开始设计对齐）+ §14.4（重构即架构设计）+ §1.2 验收
> **审查范围**: Stage 6.13 单一子阶段（lexer/reader.rs 按 02-grammar.md §1 拆分）

## CI/CD

```
cargo clean: clean
cargo test: 1881 passed, 0 failed, 2 ignored
cargo fmt --check: clean (exit 0)
cargo clippy --all-targets: 0 warnings, 0 errors
```

## §13.4 阶段开始设计对齐

依据 v3.21 §13.4，本阶段开始时查阅了 `docs/lang-design/02-grammar.md` §1（词法结构）：

- **§1.1** 字符集（UTF-8 / 空白 / 注释 / BOM）
- **§1.2** Token 分类（keyword / identifier / literal / operator / punctuation / whitespace / comment）
- **§1.3** 关键字（严格保留 + 弱保留 + MVP 保留）
- **§1.4** Identifier（XID_Start + XID_Continue + raw identifier `r#`）
- **§1.5** 整数字面量（dec / hex / oct / bin + integer_suffix）
- **§1.6** 浮点字面量（float_lit + float_suffix）
- **§1.7** 字符与字符串（char / byte / string / raw_string / byte_string / raw_byte_string + escape）
- **§1.8** 运算符与标点（operator + punctuation）
- **§1.9** 长度规则（Maximal Munch）

**偏差**：实现把 40+ lex 函数堆在单一 `reader.rs`（1537 LOC），违反 §14.4 J2 + J6。

**决策**：按 §1 的 9 个子节聚合为 4 个内聚子模块（§1.3+§1.4 → ident；§1.5+§1.6 → number；§1.7 → string；§1.1+§1.8 → operators）。

## §14.4 J1-J6 判据检查

| # | 判据 | 状态 | 说明 |
|---|------|------|------|
| J1 | 架构设计对齐 | ✅ | 新结构按 02-grammar.md §1 划分（9 子节聚合为 4 模块） |
| J2 | 单一职责 | ✅ | 每个新模块用一句话能描述职责 |
| J3 | 单向流动 | ✅ | reader.rs → {ident, number, string, operators}，无环 |
| J4 | 编译相关表达完整 | ✅ | ident/number/string/operators 各自内聚（identifier + 关键字内聚；所有数值字面量内聚；所有字符/字符串 + escape 内聚；所有运算符 + 注释内聚） |
| J5 | 阶段划分清晰 | ✅ | 所有新模块在 `src/lexer/` 下，Stage 0 阶段未变 |
| J6 | 科学合理粒度 | ✅ | reader.rs 349 LOC；子模块 123-486 LOC，全部在合理区间 |

## 拆分执行结果

```
src/lexer/
  mod.rs          (60 LOC)    — crate-level re-exports + 4 子模块声明
  reader.rs       (349 LOC)   ← Lexer struct + cursor + skip_trivia + next_token + LexError (-77.3%)
  token.rs        (390 LOC)   — Token 类型定义（不变）
  ident.rs        (123 LOC)   ← lex_raw_identifier + lex_ident + is_ident_start_byte（§1.3+§1.4）
  number.rs       (303 LOC)   ← lex_number + lex_hex/oct/bin + try_lex_number_suffix（§1.5+§1.6）
  string.rs       (486 LOC)   ← 10 个字符串/字符函数 + escape（§1.7）
  operators.rs    (372 LOC)   ← lex_doc_comment + 14 个 lex_<op>（§1.1+§1.8）
```

**reader.rs**: 1537 → **349 LOC**（-77.3%，-1188 LOC）

## 可见性策略（§16 接口隔离）

- `Lexer` struct 字段全部 `pub(super)` —— 子模块可读写游标状态
- 所有 cursor 方法（`peek`/`peek_at`/`bump`/`span_from`）`pub(super)`
- `skip_trivia` `pub(super)`（next_token 调用）
- 所有 `lex_*` 方法 `pub(super)` —— sibling 子模块可互调
- `next_token` 保持 `pub` —— 唯一对外入口（driver 调用）
- `into_errors` / `is_at_end` 保持 `pub`
- `is_ident_start_byte` `pub(super)`（reader.rs next_token 调用）
- lexer-external 代码只看到 `Lexer::new` + `Lexer::next_token` + `Lexer::into_errors` + `Lexer::is_at_end`（§16 合规）

## §23 API 命名合规

- 所有函数名保留原名（零 churn）
- 模块名遵循 `<noun>` 模式（与既有 `token.rs` 风格一致）
- 无新公共符号
- 无 `pub use X::*;` glob
- `next_token` 仍是唯一 `pub` 入口

## TD-023 累计进展

新增技术债 TD-023（Stage 6.13 引入）：lexer/reader.rs 拆分为 4 子模块，已偿还。

| Stage | reader.rs LOC | Δ |
|-------|--------------|---|
| 6.12 (baseline) | 1537 | — |
| **6.13 (architectural split)** | **349** | **-1188 (-77.3%)** |

## 七维度审查（精简版）

| 维度 | 状态 |
|------|------|
| D1 架构健康度 | ✅ 6-module 目录结构，每个模块单一职责，数据流单向 |
| D2 技术债清单 | ✅ TD-023 引入并立即偿还；TD-011/015/017/018/019/022 状态不变 |
| D3 测试覆盖 | ✅ 1881 tests 零回归 |
| D4 下一阶段就绪度 | ✅ Stage 6.14 候选（borrowck/mod.rs 1452 LOC）已识别 |
| D5 设计合理性 | ✅ §14.4 J1-J6 全部通过，§13.4 设计文档对齐 |
| D6 性能 | ✅ 无性能影响（行为等价拆分） |
| D7 文档 | ✅ plan-6.13 + gate-review-6.13 + dev-log + api-naming-standard v1.82 + RELEASE_NOTES + README + worklog |

## 委员会投票

**5/5 GO → PASS**

## 后续行动

| 优先级 | 行动 | 目标阶段 |
|--------|------|---------|
| P2 | borrowck/mod.rs 架构性拆分（1452 LOC → 按分析类别） | Stage 6.14 |
| P3 | typeck/checker.rs 拆分（1320 LOC → 按检查类别） | Stage 6.15 |
| P2 | 完整 §25.8 设计回写（全 docs/lang-design/） | Stage 6 末尾 |
| P2 | TD-015: Region inference | Stage 6+ |
| P3 | TD-018: 用户自定义 trait dyn | Stage 6+ |

---

**审查完成**: 2026-07-25
