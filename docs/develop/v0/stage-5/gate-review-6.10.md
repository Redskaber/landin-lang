# Stage 6 Gate Review Round 10 (6.10) — mir/lower expr_operand architectural split

> **审查日期**: 2026-07-25 | **版本**: v0.12.7 → v0.12.8 → v0.12.9
> **流程**: stage-committee-process.md v3.20 §17.3 时期 2 + §1.2 验收
> **审查范围**: Stage 6.10 单一子阶段（架构性重新分析 + expr_operand 提取）

## CI/CD

```
cargo clean: clean (892.7 MiB removed)
cargo test: 1881 passed, 0 failed, 2 ignored
cargo fmt --check: clean (exit 0)
cargo clippy --all-targets: 0 warnings, 0 errors
```

## 用户要求

> 继续架构性和阶段性标准化拆分（重新分析 mir/lower）
> 文件的拆分不是说只为了缩小体积，还有需要符合架构设计需求、
> 科学合理划分、其实本质上就只组织结构的设计

本 stage 严格遵循该原则：先做**架构性重新分析**，识别 mod.rs 内部
4 个职责域，再做**单一职责**拆分。

## 架构性重新分析（plan-6.10.md §2）

`mir/lower/mod.rs` 在 Stage 6.1-6.6 已拆 6 轮，从 3346 → 1980 LOC。
本轮重新分析 1980 LOC，识别出 4 个职责域：

| 域 | 行号 | LOC | 职责 |
|----|------|-----|------|
| A: 上下文基础设施 | 1-432 | 432 | MirLowerCtxt struct + impl |
| B: Body 入口点 | 439-668 | 230 | 4 个 lower_hir_body_to_mir* + 别名 |
| C: HIR→MIR 类型转换 | 670-758 | 89 | const_eval_array_len + lower_hir_ty_to_mir_ty |
| **D: 表达式降低算法** | **768-1980** | **1212** | **lower_expr_to_operand + 3 helpers** |

**关键发现**：域 D 占据 mod.rs 的 61.4%，是最大的职责混合点。
该域包含 4 个函数共同构成"HIR 表达式 → MIR operand/terminator"
完整算法，与上下文基础设施耦合度低（仅通过 `&mut MirLowerCtxt`
公开 API 交互），是教科书式高内聚低耦合的拆分边界。

## 拆分执行

```
src/mir/lower/
  mod.rs           (1980 → 772 LOC, -61.0%)  ← 上下文 + 入口点 + 类型转换工具
  expr_operand.rs  (新, 1275 LOC)            ← 表达式降低算法
  adt_layout.rs    (147 LOC, Stage 6.1)
  closure_capture.rs (175 LOC, Stage 6.2)
  pattern_bindings.rs (286 LOC, Stage 6.3)
  overflow_assert.rs (94 LOC, Stage 6.4)
  field_resolution.rs (167 LOC, Stage 6.5)
  control_flow.rs  (462 LOC, Stage 6.6)
```

**提取的 4 个函数**：

| 函数 | 原 LOC | 新可见性 | 备注 |
|------|--------|---------|------|
| `lower_expr_to_place` | 95 | `pub(crate)` | 4 个内部调用点 |
| `build_dyn_trait_call_terminator` | 35 | `pub` | 公开 API，mir/mod.rs re-export |
| `lower_expr_to_operand` | 1066 | `pub(crate)` | 巨型函数，30+ HirExprKind 变体 |
| `resolve_enum_variant` | 14 | `pub(crate)` | adt_layout/control_flow 共用 |

**mod.rs re-export**：
```rust
pub use expr_operand::build_dyn_trait_call_terminator;
pub(crate) use expr_operand::{lower_expr_to_operand, resolve_enum_variant};
```
- `pub use` 保持 `mir/mod.rs` 公开 re-export chain 不变
- `pub(crate) use` 让 sibling 模块 `control_flow.rs`、`pattern_bindings.rs`
  继续通过 `super::lower_expr_to_operand` / `super::resolve_enum_variant`
  访问，**零调用点改动**
- `lower_expr_to_place` 不 re-export（仅 expr_operand 内部使用）

## §16 接口隔离合规

✅ expr_operand 通过 `&mut MirLowerCtxt` 公开 API 交互，不访问私有字段
✅ 数据流单向：mod.rs → expr_operand → MirLowerCtxt → 辅助模块
✅ 无反向依赖（expr_operand 不调用 mod.rs 私有函数）
✅ 无循环依赖

## §23 API 命名合规

✅ 所有函数名保留原名（零 churn）
✅ 模块名 `expr_operand` 遵循 `<noun>_<noun>` 模式，与既有
   `adt_layout`、`closure_capture`、`pattern_bindings` 风格一致
✅ 无 `pub use X::*;` glob（每名显式列出）
✅ 无新公共符号（纯架构性重组）

## TD-011 累计进展

| Stage | mod.rs LOC | Δ | 累计 Δ |
|-------|-----------|---|--------|
| 5.97 (基线) | 3346 | — | — |
| 6.1 (adt_layout) | 3199 | -147 | -147 (-4.4%) |
| 6.2 (closure_capture) | 3035 | -164 | -311 (-9.3%) |
| 6.3 (pattern_bindings) | 2730 | -305 | -616 (-18.4%) |
| 6.4 (overflow_assert) | 2656 | -74 | -690 (-20.6%) |
| 6.5 (field_resolution) | 2452 | -204 | -894 (-26.7%) |
| 6.6 (control_flow) | 1980 | -472 | -1366 (-40.8%) |
| **6.10 (expr_operand)** | **772** | **-1208** | **-2574 (-76.9%)** |

🎉 **TD-011 累计 -76.9%**（从 3346 → 772 LOC），mod.rs 从超巨文件
变为骨架文件。`expr_operand.rs`（1275 LOC）作为单一职责"算法核心"
独立存在，未来 Stage 6.12+ 可按表达式类别（primary/ops/aggregate/
control/call/misc）继续细拆。

## 七维度审查

### D1. 架构健康度

✅ 8-module 目录结构（mod.rs + 7 子模块），每个模块单一职责
✅ 数据流单向，无循环
✅ mod.rs 从"巨型混合文件"变为"骨架 + 入口点"

### D2. 技术债清单

| ID | 描述 | 优先级 | 状态 | 变化 |
|----|------|--------|------|------|
| TD-011 | mir/lower/mod.rs LOC | P2 | **大幅偿还** | 1980 → 772 (-61%) |
| TD-015 | Region inference | P2 | OPEN | 无变化 |
| TD-017 | codegen/mod.rs LOC | P3 | **已关闭** (6.7-6.8) | — |
| TD-018 | dyn Trait 仅 stdlib | P3 | OPEN | 无变化 |

新增候选 TD：
- **TD-019**: `expr_operand.rs` 1275 LOC，未来需按表达式类别细拆（P3, Stage 6.12+）

### D3. API 命名标准化（§23）

无新公共符号。所有函数保留原名。模块名 `expr_operand` 合规。

### D4. 接口隔离（§16）

✅ expr_operand 仅通过 MirLowerCtxt 公开 API 交互
✅ 数据流单向 mod.rs → expr_operand → MirLowerCtxt → 辅助模块

### D5. 测试覆盖

- 总量：1881 tests（无变化）
- 测试模块：110 mods（无变化）
- 行为等价拆分，测试零回归

### D6. 文档完整性

✅ plan-6.10.md（含架构重新分析）
✅ gate-review-6.10.md（本文件）
✅ api-naming-standard.md v1.79 条目
✅ dev-log.md 追加
✅ RELEASE_NOTES.md 追加
✅ README.md 更新
✅ worklog.md 追加

### D7. CI/CD 健康

```
cargo clean: clean ✅
cargo test: 1881 passed, 0 failed, 2 ignored ✅
cargo fmt --check: clean ✅
cargo clippy --all-targets: 0 warnings, 0 errors ✅
```

## 委员会投票

**5/5 GO → PASS**

## 后续行动

| 优先级 | 行动 | 目标阶段 |
|--------|------|---------|
| P3 | TD-019: expr_operand 按表达式类别细拆 | Stage 6.12+ |
| P3 | 提取 hir_ty.rs（lower_hir_ty_to_mir_ty + const_eval_array_len） | Stage 6.11 |
| P2 | TD-015: Region inference | Stage 6+ |
| P3 | TD-018: 用户自定义 trait dyn | Stage 6+ |

---

**审查完成**: 2026-07-25
