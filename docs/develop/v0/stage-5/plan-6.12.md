# Stage 6.12 开发计划：parser.rs 架构性拆分 — 按解析类别 6 模块

> **阶段**: Stage 6.12
> **版本**: v0.13.0 → v0.13.1
> **状态**: 🟡 In Progress
> **流程**: stage-committee-process.md v3.21 §13.4（阶段开始设计对齐）+ §14.4（重构即架构设计）

## 1. 阶段开始设计对齐（§13.4 强制）

### 1.1 对应设计文档

| 设计文档 | 章节 | 用途 |
|---------|------|------|
| `docs/lang-design/02-grammar.md` | §2 Parser 概览 + §3 语法产生式 | Parser 设计基线 |
| `docs/lang-design/05-ast.md` | 全文 | AST 数据结构边界 |

### 1.2 设计意图摘要（02-grammar.md §2 + §3）

> Landin parser 是**手写 recursive descent + Pratt parser**：
> - 声明与语句：recursive descent
> - 表达式：Pratt parser（top-down operator precedence，参考 Pratt 1973）
>
> 每个 parse 函数返回 `Result<T, ParseError>`，错误恢复通过 `synthetic node` 实现。

设计文档 §3 把语法产生式明确分为 7 类：
- §3.1 Crate 与 module（item 体系）
- §3.2 Generic 与 bound
- §3.3 Type
- §3.4 表达式
- §3.5 模式
- §3.6 语句
- §3.7 use 声明

### 1.3 当前实现 vs 设计文档

#### 已对齐项

- ✅ 手写 recursive descent + Pratt（与设计一致）
- ✅ 错误恢复通过 synthetic node + skip-to-sync（与设计一致）
- ✅ Pratt 优先级表与设计 §2 一致
- ✅ 所有产生式都实现了

#### 已知偏差

- **B3 实现 ≠ 设计（结构层面）**：设计文档把语法产生式分为 7 类（§3.1-§3.7），
  但实现把所有 60+ parse 函数堆在单一 `parser.rs`（3112 LOC），违反 §14.4 J2
  （单一职责）和 J6（科学合理粒度）。

### 1.4 本阶段灰区决策

| 灰区 | 决策 | 理由 |
|------|------|------|
| 拆分粒度？ | 按 §3.1-§3.7 的 7 类对应到 6 个子模块（合并 §3.1+§3.7 为 items.rs） | 与设计文档对齐（§14.4 J1） |
| 是否拆分 Pratt 优先级表？ | 不拆，保留在 mod.rs | 设计文档明确说"Pratt 在 parser 内部" |
| 是否拆分错误恢复？ | 不拆，保留在 mod.rs（与 cursor 操作紧密耦合） | 单一职责边界（错误恢复 = cursor 操作 + sync token 识别） |
| `pub` 可见性？ | 所有 parse_* 函数改为 `pub(super)` 或 `pub(crate)` | §16 隔离——parser 外部只应看到 `parse_crate` 入口 |

## 2. §14.4 J1-J6 判据检查

### 2.1 J1 架构设计对齐 ✅

新结构按 02-grammar.md §3.1-§3.7 划分，一一对应：

| 设计文档章节 | 新模块 | 内容 |
|------------|--------|------|
| §3.1 + §3.7（item + use） | `items.rs` | parse_item + parse_fn + parse_const + parse_static + parse_struct + parse_enum + parse_trait + parse_impl + parse_type_alias + parse_extern_block_or_fn + parse_mod + parse_use + parse_use_tree + parse_outer_attrs + parse_attr_args + parse_visibility |
| §3.2（generic + bound） | `generics.rs` | parse_generics + parse_type_bounds + parse_where_clause + parse_params + parse_return_type |
| §3.3（type） | `ty.rs` | parse_ty + ty_to_path + try_parse_turbofish_or_generic_args + try_parse_generic_args |
| §3.4（expr） | `expr.rs` | parse_expr + parse_assign_expr + parse_range_expr + 13 个 Pratt 层级函数 + parse_unary_expr + parse_postfix_expr + parse_primary_expr + parse_if_expr + parse_match_expr + is_expr_start + binop_bp + assign_op + ExprSpan trait |
| §3.5（pat） | `pat.rs` | parse_pat + parse_or_pat + parse_pat_no_or + skip_delim_group + parse_path_in_pat |
| §3.6（stmt） | `stmt.rs` | parse_block + parse_let |
| §3.1 path（独立） | `path.rs` | PathContext + parse_path + parse_path_in_expr + parse_path_with_ctx + make_path |

### 2.2 J2 单一职责 ✅

每个新模块承担且仅承担一个明确的职责（用一句话能描述）：
- `items.rs` = "解析 item 级语法（fn/struct/enum/trait/impl/...）"
- `generics.rs` = "解析 generic params + bounds + where clause"
- `ty.rs` = "解析 type 语法 + generic args"
- `expr.rs` = "解析表达式（Pratt + postfix + primary）"
- `pat.rs` = "解析 pattern 语法"
- `stmt.rs` = "解析 block 与 let 语句"
- `path.rs` = "解析 path 语法（type/expr/pat 三上下文）"

### 2.3 J3 单向流动 ✅

模块依赖图：

```
mod.rs (cursor + parse_crate + recover)
  ↓ 调用
items.rs (parse_item 分发)
  ↓ 调用
generics.rs / ty.rs / path.rs / expr.rs / pat.rs / stmt.rs
```

无反向依赖：items.rs 调用 generics.rs，generics.rs 不调用 items.rs。
无循环依赖：所有子模块只通过 `Parser` 的方法调用，不互相调用。

### 2.4 J4 编译相关表达完整 ✅

每个模块的"编译相关概念"在模块内是完整的：
- `path.rs`：PathContext enum + path 解析全部内聚
- `expr.rs`：Pratt 优先级表 + 所有 Pratt 层级函数内聚
- `ty.rs`：所有 type-related 解析内聚

### 2.5 J5 阶段划分清晰 ✅

所有新模块仍在 `src/parser/` 目录下，仍是 Stage 0 阶段。不破坏 §16 阶段隔离。

### 2.6 J6 科学合理粒度 ✅

拆分后 LOC 分布（估算）：

| 模块 | 估算 LOC | 设计依据 |
|------|---------|---------|
| `mod.rs` | ~250 | cursor 操作 + parse_crate 入口 + recover + Parser struct |
| `items.rs` | ~900 | 16 个 parse_* 函数（item 体系） |
| `expr.rs` | ~1000 | 18 个 Pratt 层级 + postfix + primary + if/match |
| `ty.rs` | ~350 | parse_ty + generic args |
| `pat.rs` | ~300 | pattern 解析 |
| `path.rs` | ~200 | path 解析 + PathContext |
| `stmt.rs` | ~120 | block + let |
| `generics.rs` | ~250 | generics + bounds + where + params + return type |
| **总计** | ~3370 | （含模块头注释略增） |

每个模块均在 100-1500 LOC 合理区间，mod.rs 远低于 1500 阈值。

## 3. 拆分方案

### 3.1 目标组织结构

```
src/parser/
  mod.rs          (44 LOC, 不变)  — crate-level re-exports
  parser.rs       (~250 LOC, -92%) ← Parser struct + cursor + parse_crate + recover
  items.rs        (新, ~900 LOC)  ← item 体系（§3.1 + §3.7）
  generics.rs     (新, ~250 LOC)  ← generic + bound + where（§3.2）
  ty.rs           (新, ~350 LOC)  ← type 解析（§3.3）
  expr.rs         (新, ~1000 LOC) ← 表达式 Pratt（§3.4）
  pat.rs          (新, ~300 LOC)  ← 模式（§3.5）
  stmt.rs         (新, ~120 LOC)  ← block + let（§3.6）
  path.rs         (新, ~200 LOC)  ← path 三上下文
  error.rs        (34 LOC, 不变)
```

### 3.2 可见性策略

- `Parser` struct 字段保持 `pub(super)` 或私有 + `pub(super)` accessor
- 所有 `parse_*` 方法保持 `impl Parser` 但通过 `mod xxx;` 分文件
- Rust 允许同一 impl 块跨文件——只需在每个子模块开头写 `impl<'a> Parser<'a> { ... }`
- 关键：`peek` / `bump` / `eat` / `expect` 等 cursor 方法必须是 `pub(super)` 让子模块可调用

### 3.3 §23 API 命名合规

- 所有函数名保留原名（零 churn）
- 模块名遵循 `<noun>` 模式（与既有 `error.rs` 风格一致）
- 无新公共符号（纯架构性重组）
- `parse_crate` 仍是唯一 `pub` 入口

### 3.4 §16 接口隔离合规

- 子模块通过 `impl Parser` 方法访问 cursor，不直接读字段
- 数据流单向：mod.rs 入口 → items.rs 分发 → 各子模块 → AST 节点
- 无跨阶段调用

## 4. 风险与缓解

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| 字段可见性不足（cursor 方法被 super 调用） | 中 | 编译失败 | 把 cursor 方法和字段改 `pub(super)` |
| impl 跨文件导致方法找不到 | 低 | 编译失败 | 每个子模块独立 `impl<'a> Parser<'a> { ... }` |
| 移动函数时遗漏依赖（如 PathContext） | 中 | 编译失败 | 提取共享类型到 mod.rs 或 path.rs |
| 1881 测试回归 | 低 | 测试失败 | 行为等价拆分，逐模块迁移 + cargo test 验证 |

## 5. 验收标准（§1.2）

- [ ] `cargo clean && cargo test` — 1881 tests 全过
- [ ] `cargo fmt` — clean
- [ ] `cargo clippy --all-targets` — 0 warnings, 0 errors
- [ ] `parser/parser.rs` 降到 ~250 LOC（-92%）
- [ ] 7 个新子模块各自单一职责
- [ ] 文档：plan-6.12.md + gate-review-6.12.md + dev-log + api-naming-standard v1.81 + RELEASE_NOTES + README + worklog
- [ ] 版本 v0.13.0 → v0.13.1

## 6. 后续 Stage 6.13+ 候选

完成本轮后：

- **Stage 6.13**: lexer/reader.rs 拆分（1537 LOC → 按词法类别：ident / number / string / operator）
- **Stage 6.14**: borrowck/mod.rs 拆分（1452 LOC → 按分析类别）
- **Stage 6.15**: typeck/checker.rs 拆分（1320 LOC → 按检查类别）
- **Stage 6 末尾**: 完整 §25.8 设计回写（全 docs/lang-design/）
- **TD-015**: Region inference
- **TD-018**: 用户自定义 trait dyn 支持

---

**创建日期**: 2026-07-25
