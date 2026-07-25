# Stage 6.14 开发计划：borrowck/mod.rs 架构性拆分 — 按分析职责 4 模块

> **阶段**: Stage 6.14
> **版本**: v0.13.2 → v0.13.3
> **状态**: 🟡 In Progress
> **流程**: stage-committee-process.md v3.21 §13.4（阶段开始设计对齐）+ §14.4（重构即架构设计）

## 1. 阶段开始设计对齐（§13.4 强制）

### 1.1 对应设计文档

| 设计文档 | 章节 | 用途 |
|---------|------|------|
| `docs/lang-design/04-ownership-borrowing.md` | §2 借用规则 + §4 NLL 算法实现 + §6 借用错误诊断 | Borrow checker 设计基线 |

### 1.2 设计意图摘要（04-ownership-borrowing.md）

设计文档把 borrow checker 分为以下概念层：
- **§2 借用规则**：共享借用 vs 独占借用 + 借用检查规则 + NLL + Two-phase borrows
- **§4 NLL 算法实现**：
  - §4.1 数据结构（BorrowSet / MoveTracker / RegionInferenceContext）
  - §4.2 算法三阶段（Liveness → Maybe-initialized → Borrow analysis）
  - §4.3 Liveness analysis（前向数据流）
  - §4.4 Maybe-initialized places
  - §4.5 Move tracking
- **§6 借用错误诊断**：错误类型 + 诊断信息设计

### 1.3 当前实现 vs 设计文档

#### 已对齐项

- ✅ NLL（Non-Lexical Lifetimes）已实现（设计 §2.3）
- ✅ BorrowSet + MoveTracker 数据结构（设计 §4.1）
- ✅ Liveness via last-use map（设计 §4.3）
- ✅ Move tracking（设计 §4.5）
- ✅ 借用错误诊断（设计 §6）

#### 已知偏差

- **B3 实现 ≠ 设计（结构层面）**：设计文档把 borrow checker 分为
  BorrowSet/MoveTracker/RegionInference 等独立概念，但实现把所有
  BorrowChecker impl + NLL last-use + Copy detection + PlacePath + entry
  points 都堆在单一 `mod.rs`（1452 LOC），违反 §14.4 J2 + J6。

### 1.4 本阶段灰区决策

| 灰区 | 决策 | 理由 |
|------|------|------|
| 拆分粒度？ | 按设计 §4 的分析阶段 + 实现职责聚合为 4 个子模块 | 与设计文档对齐（§14.4 J1） |
| 是否拆分 BorrowChecker struct？ | 不拆，保留在 mod.rs | 是 borrow checker 的核心数据结构 |
| 是否拆分 tests？ | 不拆，保留在 mod.rs | 与被测代码紧密耦合 |
| `pub` 可见性？ | 现有 pub 函数保留 pub；私有辅助改 pub(super) | §16 隔离——borrowck 外部接口不变 |

## 2. §14.4 J1-J6 判据检查

### 2.1 J1 架构设计对齐 ✅

新结构按 04-ownership-borrowing.md §4 NLL 算法实现 + §6 错误诊断划分：

| 设计文档章节 | 新模块 | 内容 |
|------------|--------|------|
| §4.3 Liveness analysis | `liveness.rs` | LastUseMap + compute_last_use_map + statement_reads + rvalue_reads + operand_reads + place_root_reads + terminator_reads |
| §4.5 Move tracking + Copy 语义 | `copy_semantics.rs` | ty_is_copy + ty_is_copy_with_resolver + ty_is_copy_unified |
| §4 数据结构 + PlacePath | `place_path.rs` | PlacePath + PlaceRoot + ProjElem + impl PlacePath |
| §2 + §4 + §6 入口 + 核心检查器 | `mod.rs` | BorrowChecker struct + impl + check_mir_body/check_crate entry + tests |

### 2.2 J2 单一职责 ✅

每个新模块承担且仅承担一个明确的职责：
- `liveness.rs` = "NLL liveness 分析：计算每个 local 的 last-use 点"
- `copy_semantics.rs` = "Copy 语义判定：类型是否实现 Copy trait"
- `place_path.rs` = "PlacePath 数据结构：field-sensitive place 表示"
- `mod.rs` = "BorrowChecker 核心 + 入口点"

### 2.3 J3 单向流动 ✅

模块依赖图：

```
mod.rs (BorrowChecker + entry points)
  ↓ 调用
liveness.rs (compute_last_use_map) / copy_semantics.rs (ty_is_copy_*) / place_path.rs (PlacePath)
```

无反向依赖：子模块不调用 BorrowChecker 的方法。
无循环依赖：所有子模块是叶子模块，只依赖 MIR 类型。

### 2.4 J4 编译相关表达完整 ✅

每个模块的"编译相关概念"在模块内是完整的：
- `liveness.rs`：liveness 分析的所有 helper（reads 收集 + map 构造）内聚
- `copy_semantics.rs`：3 个 ty_is_copy* 变体内聚
- `place_path.rs`：PlacePath + PlaceRoot + ProjElem + impl 内聚

### 2.5 J5 阶段划分清晰 ✅

所有新模块仍在 `src/borrowck/` 目录下，仍是 Stage 2 阶段。不破坏 §16 阶段隔离。

### 2.6 J6 科学合理粒度 ✅

拆分后 LOC 分布（估算）：

| 模块 | 估算 LOC | 设计依据 |
|------|---------|---------|
| `mod.rs` | ~570 | BorrowChecker struct + impl + entry points + tests |
| `liveness.rs` | ~140 | LastUseMap + 6 个 reads/collection 函数 |
| `copy_semantics.rs` | ~110 | 3 个 ty_is_copy* 函数 |
| `place_path.rs` | ~90 | PlacePath + PlaceRoot + ProjElem + impl |
| **总计** | ~910 | （tests 占 ~600 LOC，纯代码 ~850 LOC） |

每个模块均在 100-1500 LOC 合理区间，mod.rs 远低于 1500 阈值。

## 3. 拆分方案

### 3.1 目标组织结构

```
src/borrowck/
  mod.rs          (~570 LOC, -61%) ← BorrowChecker struct + impl + entry points + tests
  borrow_set.rs   (341 LOC, 不变)  — BorrowSet 数据结构
  error.rs        (92 LOC, 不变)   — BorrowError 类型
  move_tracker.rs (90 LOC, 不变)   — MoveTracker 数据结构
  liveness.rs     (新, ~140 LOC)  ← NLL liveness analysis（§4.3）
  copy_semantics.rs (新, ~110 LOC) ← Copy 语义判定（§4.5 相关）
  place_path.rs   (新, ~90 LOC)   ← PlacePath 数据结构（§4 数据结构）
```

### 3.2 可见性策略（与 Stage 6.12/6.13 一致）

- `BorrowChecker` struct 字段保持现状（私有 + 通过方法访问）
- 所有现有 `pub` 函数保持 `pub`（不破坏对外 API）
- 提取的私有函数 + 新模块的函数改 `pub(super)` 或 `pub(crate)`
- `PlacePath` / `PlaceRoot` / `ProjElem` 已是 `pub`，保留

### 3.3 §23 API 命名合规

- 所有函数名保留原名（零 churn）
- 模块名遵循 `<noun>` 模式（与既有 `borrow_set.rs`、`move_tracker.rs` 风格一致）
- 无新公共符号（纯架构性重组）
- `check_mir_body` / `check_crate` 仍是入口

### 3.4 §16 接口隔离合规

- 子模块通过 MIR 数据结构交互，不访问 BorrowChecker 私有字段
- 数据流单向：mod.rs 入口 → BorrowChecker.check_mir_body → liveness/copy_semantics/place_path 辅助
- 无跨阶段调用

## 4. 风险与缓解

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| 测试模块依赖被提取的符号 | 中 | 编译失败 | 测试保留在 mod.rs，提取的符号通过 `use` 导入 |
| PlacePath 方法被 BorrowChecker 调用 | 低 | 编译失败 | PlacePath 已是 pub，直接调用 |
| 1881 测试回归 | 低 | 测试失败 | 行为等价拆分，逐模块迁移 + cargo test 验证 |

## 5. 验收标准（§1.2）

- [ ] `cargo clean && cargo test` — 1881 tests 全过
- [ ] `cargo fmt` — clean
- [ ] `cargo clippy --all-targets` — 0 warnings, 0 errors
- [ ] `borrowck/mod.rs` 降到 ~570 LOC（-61%）
- [ ] 3 个新子模块各自单一职责
- [ ] 文档：plan-6.14.md + gate-review-6.14.md + dev-log + api-naming-standard v1.83 + RELEASE_NOTES + README + worklog
- [ ] 版本 v0.13.2 → v0.13.3

## 6. 后续 Stage 6.15+ 候选

完成本轮后：

- **Stage 6.15**: typeck/checker.rs 拆分（1320 LOC → 按检查类别）
- **Stage 6 末尾**: 完整 §25.8 设计回写（全 docs/lang-design/）
- **TD-015**: Region inference
- **TD-018**: 用户自定义 trait dyn 支持

---

**创建日期**: 2026-07-25
