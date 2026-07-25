# Stage 6 Gate Review Round 12 (6.12) — parser.rs architectural split per §14.4

> **审查日期**: 2026-07-25 | **版本**: v0.13.0 → v0.13.1
> **流程**: stage-committee-process.md v3.21 §13.4（阶段开始设计对齐）+ §14.4（重构即架构设计）+ §1.2 验收
> **审查范围**: Stage 6.12 单一子阶段（parser.rs 按 02-grammar.md §3.1-§3.7 拆分）

## CI/CD

```
cargo clean: clean (890.6 MiB removed)
cargo test: 1881 passed, 0 failed, 2 ignored
cargo fmt --check: clean (exit 0)
cargo clippy --all-targets: 0 warnings, 0 errors
```

## §13.4 阶段开始设计对齐

依据 v3.21 §13.4，本阶段开始时查阅了 `docs/lang-design/02-grammar.md`：

- **§2 Parser 概览**：手写 recursive descent + Pratt parser
- **§3 语法产生式**（7 类）：
  - §3.1 Crate 与 module（item 体系）
  - §3.2 Generic 与 bound
  - §3.3 Type
  - §3.4 表达式
  - §3.5 模式
  - §3.6 语句
  - §3.7 use 声明

**偏差**：实现把 60+ parse 函数堆在单一 `parser.rs`（3112 LOC），违反 §14.4 J2 + J6。

**决策**：按 §3.1-§3.7 的 7 类对应到 7 个子模块（合并 §3.1+§3.7 为 items.rs）。

## §14.4 J1-J6 判据检查

| # | 判据 | 状态 | 说明 |
|---|------|------|------|
| J1 | 架构设计对齐 | ✅ | 新结构按 02-grammar.md §3.1-§3.7 划分，一一对应 |
| J2 | 单一职责 | ✅ | 每个新模块用一句话能描述职责 |
| J3 | 单向流动 | ✅ | mod.rs → items.rs → {generics, ty, path, expr, pat, stmt}，无环 |
| J4 | 编译相关表达完整 | ✅ | PathContext + path 解析内聚；Pratt 优先级表 + 13 层级函数内聚 |
| J5 | 阶段划分清晰 | ✅ | 所有新模块在 `src/parser/` 下，Stage 0 阶段未变 |
| J6 | 科学合理粒度 | ✅ | parser.rs 263 LOC；子模块 104-1028 LOC，全部在合理区间 |

## 拆分执行结果

```
src/parser/
  mod.rs          (56 LOC)    — crate-level re-exports + 7 子模块声明
  parser.rs       (263 LOC)   ← Parser struct + cursor + parse_crate + recover (-91.5%)
  error.rs        (34 LOC)    — ParseError 定义（不变）
  items.rs        (780 LOC)   ← 16 个 parse_* 函数（§3.1 + §3.7）+ ty_to_path helper
  expr.rs         (1028 LOC)  ← 21 个 Pratt/expr 函数 + ExprSpan trait（§3.4）
  pat.rs          (318 LOC)   ← 4 个 pattern 函数（§3.5）
  path.rs         (268 LOC)   ← 7 个 path 函数 + PathContext 引用（§3.1 path）
  generics.rs     (274 LOC)   ← 5 个 generics/bounds/where 函数（§3.2）
  ty.rs           (254 LOC)   ← parse_ty（§3.3）
  stmt.rs         (104 LOC)   ← parse_block + parse_let（§3.6）
```

**parser.rs**: 3112 → **263 LOC**（-91.5%，-2849 LOC）

## 可见性策略（§16 接口隔离）

- `Parser` struct 字段全部 `pub(super)` —— 子模块可读写游标状态
- 所有 cursor 方法（`peek`/`bump`/`eat`/`expect`/`expect_ident`/`ident_from_token`/`current_span`/`peek_at`）`pub(super)`
- `parse_crate` 保持 `pub` —— 唯一对外入口
- 所有 `parse_*` 方法 `pub(super)` —— sibling 子模块可互调
- `PathContext` enum `pub(super)` —— path.rs 引用
- `ExprSpan` trait `pub(super)` —— expr.rs 内部使用
- parser-external 代码只看到 `Parser::new` + `Parser::parse_crate` + `Parser::into_errors` + `Parser::has_errors`（§16 合规）

## §23 API 命名合规

- 所有函数名保留原名（零 churn）
- 模块名遵循 `<noun>` 模式（与既有 `error.rs` 风格一致）
- 无新公共符号
- 无 `pub use X::*;` glob
- `parse_crate` 仍是唯一 `pub` 入口

## TD-022 累计进展

新增技术债 TD-022（Stage 6.12 引入）：parser.rs 拆分为 7 子模块，已偿还。

| Stage | parser.rs LOC | Δ |
|-------|--------------|---|
| 6.11 (baseline) | 3112 | — |
| **6.12 (architectural split)** | **263** | **-2849 (-91.5%)** |

## 七维度审查（精简版）

| 维度 | 状态 |
|------|------|
| D1 架构健康度 | ✅ 8-module 目录结构，每个模块单一职责，数据流单向 |
| D2 技术债清单 | ✅ TD-022 引入并立即偿还；TD-011/015/017/018/019 状态不变 |
| D3 测试覆盖 | ✅ 1881 tests 零回归 |
| D4 下一阶段就绪度 | ✅ Stage 6.13 候选（lexer/reader.rs 1537 LOC）已识别 |
| D5 设计合理性 | ✅ §14.4 J1-J6 全部通过，§13.4 设计文档对齐 |
| D6 性能 | ✅ 无性能影响（行为等价拆分） |
| D7 文档 | ✅ plan-6.12 + gate-review-6.12 + dev-log + api-naming-standard v1.81 + RELEASE_NOTES + README + worklog |

## 委员会投票

**5/5 GO → PASS**

## 后续行动

| 优先级 | 行动 | 目标阶段 |
|--------|------|---------|
| P2 | lexer/reader.rs 架构性拆分（1537 LOC → 按词法类别） | Stage 6.13 |
| P3 | borrowck/mod.rs 拆分（1452 LOC → 按分析类别） | Stage 6.14 |
| P3 | typeck/checker.rs 拆分（1320 LOC → 按检查类别） | Stage 6.15 |
| P2 | 完整 §25.8 设计回写（全 docs/lang-design/） | Stage 6 末尾 |
| P2 | TD-015: Region inference | Stage 6+ |
| P3 | TD-018: 用户自定义 trait dyn | Stage 6+ |

---

**审查完成**: 2026-07-25
