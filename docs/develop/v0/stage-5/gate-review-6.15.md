# Stage 6 Gate Review Round 15 (6.15) — typeck/checker.rs architectural split per §14.4

> **审查日期**: 2026-07-25 | **版本**: v0.13.3 → v0.13.4
> **流程**: stage-committee-process.md v3.21 §13.4（阶段开始设计对齐）+ §14.4（重构即架构设计）+ §1.2 验收
> **审查范围**: Stage 6.15 单一子阶段（typeck/checker.rs 按 03-type-system.md §4+§8 拆分）

## CI/CD

```
cargo clean: clean
cargo test: 1881 passed, 0 failed, 2 ignored
cargo fmt --check: clean (exit 0)
cargo clippy --all-targets: 0 warnings, 0 errors
```

## §13.4 阶段开始设计对齐

依据 v3.21 §13.4，本阶段开始时查阅了 `docs/lang-design/03-type-system.md`：

- **§4 类型推导**：
  - §4.1 整体策略（constraint-based inference）
  - §4.2 Inference variable
  - §4.3 Constraint 类型
  - §4.4 Constraint 生成规则
  - §4.5 Unification 算法（在 `unify.rs`）
  - §4.6 整数 fallback
- **§8 Subtyping 规则**：coercion 矩阵
- **§9 错误信息设计**：TypeError 类型（在 `error.rs`）

**偏差**：实现把 TypeckResults + FieldTyTable + FnSigTable + type predicates + TypeChecker + entry points + tests 都堆在单一 `checker.rs`（1320 LOC），违反 §14.4 J2 + J6。

**决策**：按 §4 数据结构 + §8 type 谓词 + §4 TypeChecker 拆分为 2 个子模块。

## §14.4 J1-J6 判据检查

| # | 判据 | 状态 | 说明 |
|---|------|------|------|
| J1 | 架构设计对齐 | ✅ | 新结构按 03-type-system.md §4 数据结构 + §8 Subtyping 划分 |
| J2 | 单一职责 | ✅ | tables.rs = 数据表；predicates.rs = type 谓词；checker.rs = TypeChecker 核心 |
| J3 | 单向流动 | ✅ | checker.rs → {tables, predicates}，无环 |
| J4 | 编译相关表达完整 | ✅ | tables（3 struct + impl 内聚）；predicates（6 个 type 谓词内聚） |
| J5 | 阶段划分清晰 | ✅ | 所有新模块在 `src/typeck/` 下，Stage 2 阶段未变 |
| J6 | 科学合理粒度 | ✅ | checker.rs 1160 LOC；子模块 78-132 LOC |

## 拆分执行结果

```
src/typeck/
  mod.rs          (34 LOC)    — crate-level re-exports
  checker.rs      (1160 LOC)  ← TypeChecker struct + impl + entry points + tests (-12%)
  unify.rs        (715 LOC)   — UnificationTable（不变）
  error.rs        (62 LOC)    — TypeError 类型（不变）
  tables.rs       (78 LOC)    ← typeck 数据表（§4 数据结构）
  predicates.rs   (132 LOC)   ← type 分类谓词（§8 Subtyping）
```

**checker.rs**: 1320 → **1160 LOC**（-12%，-160 LOC）

## 可见性策略（§16 + §23 合规）

- `TypeChecker` struct + impl 保留在 checker.rs（核心数据结构）
- `check_mir_body` / `check_crate` 入口点保留 `pub`（不破坏对外 API）
- 提取的 struct + 函数在子模块中保持 `pub` 或 `pub(super)`
- mod.rs 通过 `pub use` re-export 公共符号：
  - `pub use tables::{FieldTyTable, FnSigTable, TypeckResults};`
- 所有外部调用者 **API 零变更**

## §23 API 命名合规

- 所有函数名保留原名（零 churn）
- 模块名遵循 `<noun>` 模式（与既有 `unify.rs`、`error.rs` 风格一致）
- 无新公共符号（纯架构性重组）
- mod.rs 通过 `pub use` 显式 re-export（无 glob）

## TD-025 累计进展

新增技术债 TD-025（Stage 6.15 引入）：typeck/checker.rs 拆分为 2 子模块，已偿还。

| Stage | checker.rs LOC | Δ |
|-------|---------------|---|
| 6.14 (baseline) | 1320 | — |
| **6.15 (architectural split)** | **1160** | **-160 (-12%)** |

## 七维度审查（精简版）

| 维度 | 状态 |
|------|------|
| D1 架构健康度 | ✅ 5-module 目录结构，每个模块单一职责，数据流单向 |
| D2 技术债清单 | ✅ TD-025 引入并立即偿还；TD-011/015/017/018/019/022/023/024 状态不变 |
| D3 测试覆盖 | ✅ 1881 tests 零回归 |
| D4 下一阶段就绪度 | ✅ Stage 6 架构性拆分基本完成；下一步是 §25.8 完整设计回写 |
| D5 设计合理性 | ✅ §14.4 J1-J6 全部通过，§13.4 设计文档对齐 |
| D6 性能 | ✅ 无性能影响（行为等价拆分） |
| D7 文档 | ✅ plan-6.15 + gate-review-6.15 + dev-log + api-naming-standard v1.84 + RELEASE_NOTES + README + worklog |

## 委员会投票

**5/5 GO → PASS**

## 后续行动

| 优先级 | 行动 | 目标阶段 |
|--------|------|---------|
| P2 | 完整 §25.8 设计回写（全 docs/lang-design/） | Stage 6 末尾 |
| P2 | TD-015: Region inference | Stage 6+ |
| P3 | TD-018: 用户自定义 trait dyn | Stage 6+ |

---

**审查完成**: 2026-07-25
