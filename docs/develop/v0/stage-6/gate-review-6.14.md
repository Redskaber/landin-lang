# Stage 6 Gate Review Round 14 (6.14) — borrowck/mod.rs architectural split per §14.4

> **审查日期**: 2026-07-25 | **版本**: v0.13.2 → v0.13.3
> **流程**: stage-committee-process.md v3.21 §13.4（阶段开始设计对齐）+ §14.4（重构即架构设计）+ §1.2 验收
> **审查范围**: Stage 6.14 单一子阶段（borrowck/mod.rs 按 04-ownership-borrowing.md §4 拆分）

## CI/CD

```
cargo clean: clean
cargo test: 1881 passed, 0 failed, 2 ignored
cargo fmt --check: clean (exit 0)
cargo clippy --all-targets: 0 warnings, 0 errors
```

## §13.4 阶段开始设计对齐

依据 v3.21 §13.4，本阶段开始时查阅了 `docs/lang-design/04-ownership-borrowing.md`：

- **§2** 借用规则（共享/独占借用 + NLL + Two-phase borrows）
- **§4** NLL 算法实现：
  - §4.1 数据结构（BorrowSet / MoveTracker / RegionInferenceContext）
  - §4.2 算法三阶段（Liveness → Maybe-initialized → Borrow analysis）
  - §4.3 Liveness analysis
  - §4.4 Maybe-initialized places
  - §4.5 Move tracking
- **§6** 借用错误诊断

**偏差**：实现把 BorrowChecker impl + NLL last-use + Copy detection + PlacePath + entry points + tests 都堆在单一 `mod.rs`（1452 LOC），违反 §14.4 J2 + J6。

**决策**：按 §4 的分析阶段 + 实现职责聚合为 3 个子模块。

## §14.4 J1-J6 判据检查

| # | 判据 | 状态 | 说明 |
|---|------|------|------|
| J1 | 架构设计对齐 | ✅ | 新结构按 04-ownership-borrowing.md §4 NLL 算法实现划分 |
| J2 | 单一职责 | ✅ | 每个新模块用一句话能描述职责 |
| J3 | 单向流动 | ✅ | mod.rs → {liveness, copy_semantics, place_path}，无环 |
| J4 | 编译相关表达完整 | ✅ | liveness（reads 收集 + map 构造）内聚；copy_semantics（3 个 ty_is_copy*）内聚；place_path（PlacePath + impl）内聚 |
| J5 | 阶段划分清晰 | ✅ | 所有新模块在 `src/borrowck/` 下，Stage 2 阶段未变 |
| J6 | 科学合理粒度 | ✅ | mod.rs 1146 LOC（含 ~600 tests）；子模块 109-124 LOC |

## 拆分执行结果

```
src/borrowck/
  mod.rs            (1146 LOC)  ← BorrowChecker struct + impl + entry points + tests (-21%)
  borrow_set.rs     (341 LOC)   — BorrowSet 数据结构（不变）
  error.rs          (92 LOC)    — BorrowError 类型（不变）
  move_tracker.rs   (90 LOC)    — MoveTracker 数据结构（不变）
  liveness.rs       (109 LOC)   ← NLL liveness analysis（§4.3）
  copy_semantics.rs (124 LOC)   ← Copy 语义判定（§4.5 相关）
  place_path.rs     (112 LOC)   ← PlacePath 数据结构（§4 数据结构）
```

**mod.rs**: 1452 → **1146 LOC**（-21%，-306 LOC；纯代码 ~550 LOC，~600 LOC tests）

## 可见性策略（§16 接口隔离）

- `BorrowChecker` struct + impl 保留在 mod.rs（核心数据结构）
- `check_mir_body` / `check_crate` 入口点保留 `pub`（不破坏对外 API）
- 提取的函数 + 类型在子模块中保持 `pub`，mod.rs 通过 `pub use` re-export
- 所有现有 `pub` 符号的对外可见性不变（§23 + §16 合规）

## §23 API 命名合规

- 所有函数名保留原名（零 churn）
- 模块名遵循 `<noun>` 模式（与既有 `borrow_set.rs`、`move_tracker.rs` 风格一致）
- 无新公共符号（纯架构性重组）
- `check_mir_body` / `check_crate` 仍是入口
- mod.rs 通过 `pub use` 显式 re-export（无 glob）

## TD-024 累计进展

新增技术债 TD-024（Stage 6.14 引入）：borrowck/mod.rs 拆分为 3 子模块，已偿还。

| Stage | mod.rs LOC | Δ |
|-------|-----------|---|
| 6.13 (baseline) | 1452 | — |
| **6.14 (architectural split)** | **1146** | **-306 (-21%)** |

## 七维度审查（精简版）

| 维度 | 状态 |
|------|------|
| D1 架构健康度 | ✅ 6-module 目录结构，每个模块单一职责，数据流单向 |
| D2 技术债清单 | ✅ TD-024 引入并立即偿还；TD-011/015/017/018/019/022/023 状态不变 |
| D3 测试覆盖 | ✅ 1881 tests 零回归 |
| D4 下一阶段就绪度 | ✅ Stage 6.15 候选（typeck/checker.rs 1320 LOC）已识别 |
| D5 设计合理性 | ✅ §14.4 J1-J6 全部通过，§13.4 设计文档对齐 |
| D6 性能 | ✅ 无性能影响（行为等价拆分） |
| D7 文档 | ✅ plan-6.14 + gate-review-6.14 + dev-log + api-naming-standard v1.83 + RELEASE_NOTES + README + worklog |

## 委员会投票

**5/5 GO → PASS**

## 后续行动

| 优先级 | 行动 | 目标阶段 |
|--------|------|---------|
| P2 | typeck/checker.rs 架构性拆分（1320 LOC → 按检查类别） | Stage 6.15 |
| P2 | 完整 §25.8 设计回写（全 docs/lang-design/） | Stage 6 末尾 |
| P2 | TD-015: Region inference | Stage 6+ |
| P3 | TD-018: 用户自定义 trait dyn | Stage 6+ |

---

**审查完成**: 2026-07-25
