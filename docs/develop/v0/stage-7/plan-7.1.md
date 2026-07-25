# Stage 7.1 开发计划：Region inference 基础设施 (TD-015 step 1)

> **阶段**: Stage 7.1（Stage 7 首个子阶段）
> **版本**: v0.14.0 → v0.14.1
> **状态**: 🟡 In Progress
> **流程**: stage-committee-process.md v3.21 §13.4（阶段开始设计对齐）+ §14.4（重构即架构设计）

## 1. 阶段开始设计对齐（§13.4 强制）

### 1.1 对应设计文档

| 设计文档 | 章节 | 用途 |
|---------|------|------|
| `docs/lang-design/04-ownership-borrowing.md` | §3 生命周期系统 + §4.6 NLL 完整规范 | Region inference 设计基线 |
| `docs/lang-design/06-mir.md` | §2 顶层结构（Region 类型） | MIR Region 数据结构 |

### 1.2 设计意图摘要（04-ownership-borrowing.md §4.6）

设计文档 §4.6 定义了完整的 NLL + region inference 算法：

- **§4.6.1 Universal region**：函数签名中的 `'a`、`'b`、`'static`（不可推断的）
- **§4.6.2 Implied bounds**：`&'a T` 隐含 `T: 'a`
- **§4.6.3 Universe 机制**：HRTB `for<'a>` 创建新 universe
- **§4.6.4 Type tests**：验证 `T: 'a` 约束
- **§4.6.5 SCC 压缩**：region constraint graph 用 SCC 压缩
- **§4.6.6 RegionInferenceContext**：完整数据结构

```rust
struct RegionInferenceContext {
    universal_regions: Vec<Region>,          // 函数签名的 'a, 'b, 'static
    region_defs: Vec<RegionInfo>,            // 所有 region 的定义
    constraints: Vec<OutlivesConstraint>,    // 'a: 'b 约束
    type_tests: Vec<TypeTest>,               // T: 'a 验证
    universe_causes: Vec<UniverseCause>,     // universe 创建原因
    sccs: Sccs<Region>,                      // SCC 压缩
    scc_values: IndexVec<Scc, RegionSet>,    // 每个 SCC 的点集
}
```

### 1.3 当前实现 vs 设计文档

#### 已对齐项

- ✅ `Region` enum（`Static` / `Var(RegionVid)` / `Erased`）— 设计 §4.1
- ✅ `RegionVid` — 设计 §4.1
- ✅ NLL liveness analysis（`borrowck::liveness::compute_last_use_map`）— 设计 §4.3
- ✅ Move tracking — 设计 §4.5

#### 已知偏差（Stage 6.18 §25.8 回写）

- **B1（实现 < 设计）**：region inference 完全未实现（TD-015）
- **B1**：universal region / implied bounds / universe / type tests / SCC 全部未实现
- **B3**：当前 NLL 用简化的 last-use map，不做 region constraint solving

### 1.4 本阶段灰区决策

| 灰区 | 决策 | 理由 |
|------|------|------|
| 拆分粒度？ | Stage 7.1 只做数据结构 + constraint 收集，不做推理算法 | 推理算法复杂，分阶段降低风险 |
| 是否修改现有 Region 类型？ | 不修改，保持向后兼容 | 现有 `Region::Var(RegionVid)` 已满足 |
| 是否修改 borrowck？ | 不修改，新增独立 `region_inference` 模块 | §16 隔离 — 新功能独立模块 |
| `pub` 可见性？ | 新增类型 `pub(crate)`，未来需要时升级 | §16 隔离 |
| 测试策略？ | 新增单元测试 + 1881 tests 零回归 | 行为等价（新功能不激活） |

## 2. §14.4 J1-J6 判据检查（新增模块）

### 2.1 J1 架构设计对齐 ✅

新模块按 04-ownership-borrowing.md §4.6 设计：

| 设计 § | 新模块内容 |
|--------|----------|
| §4.6.1 | `UniversalRegion` + `RegionInfo` |
| §4.6.2 | `OutlivesConstraint` + constraint 收集 |
| §4.6.4 | `TypeTest` |
| §4.6.6 | `RegionInferenceContext` 完整数据结构 |

### 2.2 J2 单一职责 ✅

`region_inference.rs` = "Region inference 数据结构 + constraint 收集"

### 2.3 J3 单向流动 ✅

```
borrowck (现有 NLL)
  ↓ 未来调用
region_inference (新模块)
  ↓ 读取
MirBody (现有)
```

### 2.4 J4 编译相关表达完整 ✅

所有 region inference 概念在模块内完整。

### 2.5 J5 阶段划分清晰 ✅

新模块在 `src/borrowck/` 下，Stage 2 阶段。

### 2.6 J6 科学合理粒度 ✅

估算 ~400 LOC，合理区间。

## 3. 拆分方案

### 3.1 目标组织结构

```
src/borrowck/
  mod.rs            (1146 LOC)  — BorrowChecker（不变）
  borrow_set.rs     (341 LOC)   — 不变
  error.rs          (92 LOC)    — 不变
  move_tracker.rs   (90 LOC)    — 不变
  liveness.rs       (109 LOC)   — 不变
  copy_semantics.rs (124 LOC)   — 不变
  place_path.rs     (112 LOC)   — 不变
  region_inference.rs (新, ~400 LOC) ← TD-015 step 1: 数据结构 + constraint 收集
```

### 3.2 新增内容

1. **`RegionInfo`**：region 定义信息（universal / inference / placeholder）
2. **`OutlivesConstraint`**：`'a: 'b` 约束（subset / sup）
3. **`TypeTest`**：`T: 'a` 验证
4. **`UniverseCause`**：universe 创建原因
5. **`RegionInferenceContext`**：完整数据结构（§4.6.6）
6. **Constraint 收集 API**：`add_universal_region` / `add_outlives_constraint` / `add_type_test`
7. **单元测试**：验证数据结构 + constraint 收集

### 3.3 §23 API 命名合规

- 类型名：`RegionInfo` / `OutlivesConstraint` / `TypeTest` / `UniverseCause` / `RegionInferenceContext`
- 函数名：`add_*` / `new_*` / `collect_*`
- 模块名：`region_inference`（`<noun>_<noun>` 模式）

### 3.4 §16 接口隔离合规

- 新模块通过 `MirBody` 数据结构交互
- 不修改现有 borrowck 代码
- 不激活新功能（仅数据结构 + 测试）

## 4. 风险与缓解

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| 新增类型与现有 Region 冲突 | 低 | 编译失败 | 新类型用独立命名（`RegionInfo` vs `Region`） |
| 1881 测试回归 | 极低 | 测试失败 | 新功能不激活，纯数据结构 + 测试 |
| 设计文档与实现不一致 | 中 | 未来 bug | 严格按 §4.6 设计 |

## 5. 验收标准（§1.2）

- [ ] `cargo clean && cargo test` — 1881+ tests 全过（新增 region_inference 单元测试）
- [ ] `cargo fmt` — clean
- [ ] `cargo clippy --all-targets` — 0 warnings, 0 errors
- [ ] 新增 `src/borrowck/region_inference.rs` ~400 LOC
- [ ] 文档：plan-7.1.md + gate-review-7.1.md + dev-log + api-naming-standard v1.88 + RELEASE_NOTES + README + worklog
- [ ] 版本 v0.14.0 → v0.14.1

## 6. 后续 Stage 7.2+ 候选

完成本轮后：

- **Stage 7.2**: Region inference 算法（不动点迭代 + universal region 检查）— TD-015 step 2
- **Stage 7.3**: Implied bounds + type tests — TD-015 step 3
- **Stage 7.4**: Universe 机制 + SCC 压缩 — TD-015 step 4
- **Stage 7.5**: 集成到 borrowck（替换简化 NLL）— TD-015 step 5
- **TD-018**: 用户自定义 trait dyn 支持

---

**创建日期**: 2026-07-25
