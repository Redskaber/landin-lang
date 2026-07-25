# Stage 6.15 开发计划：typeck/checker.rs 架构性拆分 — 按检查职责 3 模块

> **阶段**: Stage 6.15
> **版本**: v0.13.3 → v0.13.4
> **状态**: 🟡 In Progress
> **流程**: stage-committee-process.md v3.21 §13.4（阶段开始设计对齐）+ §14.4（重构即架构设计）

## 1. 阶段开始设计对齐（§13.4 强制）

### 1.1 对应设计文档

| 设计文档 | 章节 | 用途 |
|---------|------|------|
| `docs/lang-design/03-type-system.md` | §4 类型推导 + §8 Subtyping 规则 + §9 错误信息设计 | Type checker 设计基线 |

### 1.2 设计意图摘要（03-type-system.md）

设计文档把 type checker 分为以下概念层：
- **§4 类型推导**：
  - §4.1 整体策略（constraint-based inference）
  - §4.2 Inference variable
  - §4.3 Constraint 类型
  - §4.4 Constraint 生成规则
  - §4.5 Unification 算法
  - §4.6 整数 fallback
- **§8 Subtyping 规则**：coercion 矩阵
- **§9 错误信息设计**：TypeError 类型

### 1.3 当前实现 vs 设计文档

#### 已对齐项

- ✅ Constraint-based inference（设计 §4.1）
- ✅ Inference variable（设计 §4.2，使用 `InferVar::TyVar/IntVar/FloatVar`）
- ✅ Unification 算法（设计 §4.5，在 `unify.rs`）
- ✅ Coercion 矩阵（设计 §8，`can_coerce` 函数）
- ✅ TypeError 类型（设计 §9，在 `error.rs`）

#### 已知偏差

- **B3 实现 ≠ 设计（结构层面）**：设计文档把 type checker 分为
  TypeChecker + FieldTyTable/FnSigTable + type predicates + entry points
  等独立概念，但实现把所有这些都堆在单一 `checker.rs`（1320 LOC），
  违反 §14.4 J2 + J6。

### 1.4 本阶段灰区决策

| 灰区 | 决策 | 理由 |
|------|------|------|
| 拆分粒度？ | 按设计 §4 + §8 + 实现职责聚合为 2 个子模块 | 与设计文档对齐（§14.4 J1） |
| 是否拆分 TypeChecker struct？ | 不拆，保留在 checker.rs | 是 type checker 的核心数据结构 |
| 是否拆分 tests？ | 不拆，保留在 checker.rs | 与被测代码紧密耦合 |
| 是否拆分 TypeckResults + FieldTyTable + FnSigTable？ | 提取到 `tables.rs` | 都是 typeck 的数据表，单一职责 |
| 是否拆分 type predicates？ | 提取到 `predicates.rs` | 都是 type 分类谓词，单一职责 |
| `pub` 可见性？ | 现有 pub 函数保留 pub；私有辅助改 pub(super) | §16 隔离——typeck 外部接口不变 |

## 2. §14.4 J1-J6 判据检查

### 2.1 J1 架构设计对齐 ✅

新结构按 03-type-system.md §4 类型推导 + §8 Subtyping 划分：

| 设计文档章节 | 新模块 | 内容 |
|------------|--------|------|
| §4 类型推导数据结构 | `tables.rs` | TypeckResults + FieldTyTable + FnSigTable |
| §8 Subtyping + type 谓词 | `predicates.rs` | is_arithmetic_ty + is_concrete_int_or_float + is_negatable_ty + is_notable_ty + is_shift_count_ty + can_coerce |
| §4 TypeChecker + §9 entry | `checker.rs` | TypeChecker struct + impl + check_mir_body/check_crate + tests |

### 2.2 J2 单一职责 ✅

每个新模块承担且仅承担一个明确的职责：
- `tables.rs` = "typeck 数据表：TypeckResults + FieldTyTable + FnSigTable"
- `predicates.rs` = "type 分类谓词：arithmetic/negatable/notable/shift + coercion"
- `checker.rs` = "TypeChecker 核心 + 入口点"

### 2.3 J3 单向流动 ✅

模块依赖图：

```
checker.rs (TypeChecker + entry points)
  ↓ 调用
tables.rs (data tables) / predicates.rs (type predicates)
```

无反向依赖：子模块不调用 TypeChecker 的方法。
无循环依赖：所有子模块是叶子模块，只依赖 MIR/AST 类型。

### 2.4 J4 编译相关表达完整 ✅

每个模块的"编译相关概念"在模块内是完整的：
- `tables.rs`：3 个数据表 struct + impl 内聚
- `predicates.rs`：6 个 type 谓词函数内聚
- `checker.rs`：TypeChecker + 所有 check_*/infer_* 方法内聚

### 2.5 J5 阶段划分清晰 ✅

所有新模块仍在 `src/typeck/` 目录下，仍是 Stage 2 阶段。不破坏 §16 阶段隔离。

### 2.6 J6 科学合理粒度 ✅

拆分后 LOC 分布（估算）：

| 模块 | 估算 LOC | 设计依据 |
|------|---------|---------|
| `checker.rs` | ~1080 | TypeChecker struct + impl + entry points + tests |
| `tables.rs` | ~60 | TypeckResults + FieldTyTable + FnSigTable (3 struct + 3 impl) |
| `predicates.rs` | ~150 | 6 个 type 谓词函数 |
| **总计** | ~1290 | （tests 占 ~240 LOC，纯代码 ~1080 LOC） |

每个模块均在 60-1500 LOC 合理区间。

## 3. 拆分方案

### 3.1 目标组织结构

```
src/typeck/
  mod.rs          (31 LOC, 不变)   — crate-level re-exports
  checker.rs      (~1080 LOC, -18%) ← TypeChecker struct + impl + entry points + tests
  unify.rs        (715 LOC, 不变)   — UnificationTable
  error.rs        (62 LOC, 不变)    — TypeError 类型
  tables.rs       (新, ~60 LOC)    ← typeck 数据表（§4 数据结构）
  predicates.rs   (新, ~150 LOC)   ← type 分类谓词（§8 Subtyping）
```

### 3.2 可见性策略（与 Stage 6.14 一致）

- `TypeChecker` struct + impl 保留在 checker.rs（核心数据结构）
- `check_mir_body` / `check_crate` 入口点保留 `pub`（不破坏对外 API）
- 提取的 struct + 函数在子模块中保持 `pub`，checker.rs 通过 `use` 导入
- mod.rs 通过 `pub use` re-export 公共符号

### 3.3 §23 API 命名合规

- 所有函数名保留原名（零 churn）
- 模块名遵循 `<noun>` 模式（与既有 `unify.rs`、`error.rs` 风格一致）
- 无新公共符号（纯架构性重组）
- mod.rs 通过 `pub use` 显式 re-export（无 glob）

### 3.4 §16 接口隔离合规

- 子模块通过 MIR/AST 数据结构交互，不访问 TypeChecker 私有字段
- 数据流单向：checker.rs 入口 → TypeChecker.check_mir_body → tables/predicates 辅助
- 无跨阶段调用

## 4. 风险与缓解

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| 测试模块依赖被提取的符号 | 中 | 编译失败 | 测试保留在 checker.rs，提取的符号通过 `use` 导入 |
| mod.rs re-export 链断裂 | 低 | 编译失败 | mod.rs 显式 `pub use` 所有公共符号 |
| 1881 测试回归 | 低 | 测试失败 | 行为等价拆分，逐模块迁移 + cargo test 验证 |

## 5. 验收标准（§1.2）

- [ ] `cargo clean && cargo test` — 1881 tests 全过
- [ ] `cargo fmt` — clean
- [ ] `cargo clippy --all-targets` — 0 warnings, 0 errors
- [ ] `typeck/checker.rs` 降到 ~1080 LOC（-18%）
- [ ] 2 个新子模块各自单一职责
- [ ] 文档：plan-6.15.md + gate-review-6.15.md + dev-log + api-naming-standard v1.84 + RELEASE_NOTES + README + worklog
- [ ] 版本 v0.13.3 → v0.13.4

## 6. 后续 Stage 6.16+ 候选

完成本轮后：

- **Stage 6 末尾**: 完整 §25.8 设计回写（全 docs/lang-design/）
- **TD-015**: Region inference
- **TD-018**: 用户自定义 trait dyn 支持

---

**创建日期**: 2026-07-25
