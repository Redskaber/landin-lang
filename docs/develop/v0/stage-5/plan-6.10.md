# Stage 6.10 开发计划：mir/lower 重新分析 + expr_operand 算法域拆分

> **阶段**: Stage 6.10
> **版本**: v0.12.8 → v0.12.9
> **状态**: 🟡 In Progress

## 1. 背景

用户明确要求"重新分析 mir/lower"，并强调：

> 文件的拆分不是说只为了缩小体积，还有需要符合架构设计需求、
> 科学合理划分、其实本质上就只组织结构的设计

Stage 6.1-6.6 已对 `mir/lower/mod.rs` 做了 6 轮拆分，从 3346 LOC 降到
1980 LOC（TD-011 累计 -40.8%）。但 1980 LOC 仍然偏大，且内部职责混合。
本轮进行**架构性重新分析**，识别 mod.rs 内部仍然存在的职责边界，再做
一次科学合理的拆分。

## 2. mir/lower 架构重新分析

### 2.1 当前 mod.rs（1980 LOC）的职责分布

通过对 mod.rs 逐段分析，识别出 **4 个职责域**：

| 域 | 行号范围 | LOC | 职责描述 |
|----|---------|-----|---------|
| **A: 上下文基础设施** | 1-432 | 432 | MirLowerCtxt struct + impl（new/fresh_*/new_local*/local_of/terminate/push_assign/lit_to_const/lower_bin_op/lower_un_op） |
| **B: Body 入口点** | 439-668 | 230 | 4 个 lower_hir_body_to_mir* + 2 个 lower_body* 别名 |
| **C: HIR→MIR 类型转换** | 670-758 | 89 | const_eval_array_len + lower_hir_ty_to_mir_ty（pub(crate)，多模块共用） |
| **D: 表达式降低算法** | 768-1980 | 1212 | lower_expr_to_place + build_dyn_trait_call_terminator + lower_expr_to_operand（巨型函数） + resolve_enum_variant |
| **总计** | — | 1963 | （+17 LOC 模块声明/imports） |

### 2.2 各域之间的依赖关系

```
   [driver.rs, typeck, codegen]
              ↑
       ┌──────┴──────┐
       │  域 B 入口   │ ← 公开 API：lower_hir_body_to_mir*
       └──────┬──────┘
              │ 调用
       ┌──────┴──────┐
       │  域 A 上下文 │ ← MirLowerCtxt + impl 方法
       └──────┬──────┘
              │ 调用
       ┌──────┴──────┐
       │  域 D 算法   │ ← lower_expr_to_operand + helpers
       └──────┬──────┘
              │ 调用
       ┌──────┴──────┐
       │  域 C 工具   │ ← lower_hir_ty_to_mir_ty（pub(crate)）
       └─────────────┘
```

数据流单向：B → A → D → C，无循环。

### 2.3 已有子模块（Stage 6.1-6.6 提取）

| 模块 | LOC | 职责 |
|------|-----|------|
| adt_layout.rs | 147 | ADT 字段类型提取 |
| closure_capture.rs | 175 | 闭包捕获分析 |
| control_flow.rs | 462 | 控制流降低（block/if/match/short_circuit/deref） |
| field_resolution.rs | 167 | 字段索引/类型解析 |
| overflow_assert.rs | 94 | 溢出/除零检查发射 |
| pattern_bindings.rs | 286 | pattern 变量绑定收集 |
| **小计** | **1331** | 已提取的辅助算法 |

### 2.4 关键发现：mod.rs 内部最大块是"表达式降低算法"

域 D（1212 LOC）占据 mod.rs 的 **61.4%**，是当前最大的职责混合点。
该域包含：

1. `lower_expr_to_place`（95 LOC）— 表达式 → Place（可赋值位置）
2. `build_dyn_trait_call_terminator`（35 LOC）— dyn Trait 调用终结符构造
3. `lower_expr_to_operand`（1066 LOC）— **巨型函数**，处理 30+ 种
   HirExprKind 变体（Lit/Path/Binary/Unary/Call/MethodCall/Closure/...）
4. `resolve_enum_variant`（14 LOC）— enum 变体索引/字段类型解析

这 4 个函数共同构成"HIR 表达式树 → MIR operand/terminator"的完整算法，
是 mir/lower 的算法核心。它们与上下文基础设施（域 A）和 body 入口
（域 B）耦合度低：
- 输入：`&mut MirLowerCtxt` + `&HirExpr`
- 输出：`LocalId` / `Place` / `Terminator`
- 不操作 MirLowerCtxt 的内部字段（仅通过 pub API 调用）

### 2.5 用户原则验证

> "文件的拆分不是说只为了缩小体积，还有需要符合架构设计需求、
>  科学合理划分、其实本质上就只组织结构的设计"

本轮拆分严格遵循该原则：

✅ **符合架构设计需求**：将"表达式降低算法"作为独立模块，与"上下文
   基础设施"分离。这是编译器架构中标准的"算法/数据结构分离"模式
   （参考 rustc 的 `rustc_mir_build::thir::expr` 与 `LoweringContext`
   分离）。

✅ **科学合理划分**：4 个函数共同构成一个完整的"表达式降低"职责，
   内聚度高；与上下文基础设施通过 `&mut MirLowerCtxt` 公开 API 交互，
   耦合度低。这是高内聚低耦合的教科书式划分。

✅ **组织结构设计**：本轮之后，mir/lower 目录的组织结构变为：
   - `mod.rs` = 上下文 + 入口点（**骨架**）
   - `expr_operand.rs` = 表达式降低算法（**算法核心**）
   - `hir_ty.rs`（未来 Stage 6.11 候选）= HIR→MIR 类型转换（**工具层**）
   - 6 个现有辅助模块 = 各专项算法（**辅助算法层**）

## 3. 拆分方案

### 3.1 目标组织结构

```
src/mir/lower/
  mod.rs              — 上下文基础设施 + Body 入口点 + 类型转换工具（~770 LOC）
  expr_operand.rs     — 表达式降低算法（~1212 LOC）← Stage 6.10 新增
  adt_layout.rs       — ADT 字段类型提取（147 LOC，Stage 6.1）
  closure_capture.rs  — 闭包捕获分析（175 LOC，Stage 6.2）
  pattern_bindings.rs — pattern 变量绑定收集（286 LOC，Stage 6.3）
  overflow_assert.rs  — 溢出/除零检查发射（94 LOC，Stage 6.4）
  field_resolution.rs — 字段索引/类型解析（167 LOC，Stage 6.5）
  control_flow.rs     — 控制流降低（462 LOC，Stage 6.6）
```

### 3.2 提取内容

从 `mod.rs` 提取到 `expr_operand.rs`：

| 函数 | 可见性 | LOC | 备注 |
|------|--------|-----|------|
| `lower_expr_to_place` | `pub(super)` | 95 | mod.rs 内部 + expr_operand 内部使用 |
| `build_dyn_trait_call_terminator` | `pub` | 35 | 已是 pub（公开 API） |
| `lower_expr_to_operand` | `pub(super)` | 1066 | 巨型函数，body lowering 调用 |
| `resolve_enum_variant` | `pub(crate)` | 14 | 已是 pub(crate)（adt_layout/control_flow 也用） |

### 3.3 接口设计

**expr_operand.rs** 对外暴露：
```rust
pub fn build_dyn_trait_call_terminator(...) -> Terminator;
pub(super) fn lower_expr_to_operand(cx: &mut MirLowerCtxt, expr: &HirExpr) -> LocalId;
pub(super) fn lower_expr_to_place(cx: &mut MirLowerCtxt, expr: &HirExpr) -> Place;
pub(crate) fn resolve_enum_variant(...) -> Option<(u32, Vec<Ty>)>;
```

**mod.rs** 中 `mod expr_operand;` + `use expr_operand::*;`（仅 module 内可见）

调用点更新：
- `mod.rs` 中 `lower_hir_body_to_mir_full*` 调用 `lower_expr_to_operand`
  → 现在通过 `expr_operand::lower_expr_to_operand` 调用
- `mod.rs` 中其他对这 4 个函数的内部调用同步更新

### 3.4 §23 API 命名合规

- 所有函数名沿用既有命名，不引入新符号
- 模块名 `expr_operand` 遵循 `<noun>_<noun>` 模式（与 `adt_layout`、
  `closure_capture`、`pattern_bindings` 等既有子模块风格一致）
- 无新 `pub use` glob，仅按需 `use`

### 3.5 §16 接口隔离合规

- expr_operand 通过 `&mut MirLowerCtxt` 公开 API 交互，不访问其私有字段
- 数据流单向：mod.rs → expr_operand（调用算法）→ MirLowerCtxt（执行）
- 无反向依赖（expr_operand 不调用 mod.rs 的私有函数）

## 4. 风险与缓解

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| 函数可见性不足（pub(super) vs pub(crate)） | 中 | 编译失败 | 先用 pub(crate)，CI 验证后降到 pub(super) |
| Import 路径遗漏 | 中 | 编译失败 | 用 grep 双向检查所有调用点 |
| resolve_enum_variant 被 adt_layout/control_flow 调用 | 低 | 编译失败 | 保留 pub(crate)，re-export 自 mod.rs |
| 巨型 match 提取时手抖 | 低 | 行为变化 | 提取整段（不加不减一行），CI test 1881 应全过 |

## 5. 验收标准（§1.2）

- [ ] `cargo clean && cargo test` — 1881 tests 全过
- [ ] `cargo fmt` — clean
- [ ] `cargo clippy --all-targets` — 0 warnings, 0 errors
- [ ] mod.rs 降到 ~770 LOC（-61%）
- [ ] expr_operand.rs ~1212 LOC，单一职责
- [ ] 文档：plan-6.10.md + gate-review-6.10.md + dev-log 更新
- [ ] api-naming-standard.md v1.79 条目
- [ ] RELEASE_NOTES.md + README.md 更新
- [ ] worklog.md 追加 stage6.10 段

## 6. 后续 Stage 6.11+ 候选

完成本轮后，mir/lower 目录结构清晰，未来候选：

- **Stage 6.11**: 提取 `hir_ty.rs`（const_eval_array_len + lower_hir_ty_to_mir_ty，~90 LOC）
  — 类型转换工具域，独立性强
- **Stage 6.12+**: 拆分 `expr_operand.rs` 巨型 match，按表达式类别分文件：
  - `expr/primary.rs` — Lit + Path
  - `expr/ops.rs` — Binary + Unary + Field + Index + AddrOf + Cast
  - `expr/aggregate.rs` — Tuple + Unit + Array + Repeat + Struct + Closure
  - `expr/control.rs` — Return + Loop + While + For + Break + Continue + Try
  - `expr/call.rs` — Call + MethodCall
  - `expr/misc.rs` — Range + MacroCall + Unsafe + Assign

- **TD-015**: Region inference（Stage 6+ 优先级 P2）
- **TD-018**: 用户自定义 trait dyn 支持（Stage 6+ 优先级 P3）

---

**创建日期**: 2026-07-25
