# Stage 6.5 开发计划(回填):mir/lower/mod.rs 拆分 — field_resolution 提取(TD-011 第五步)

> **状态**: ✅ Complete (本 plan 于 Stage 12.9 从 `gate-review-6.5.md` 回填重建)
> **流程**: stage-committee-process.md v3.20 §17.3 时期 2(原始执行);v3.21 §14.4 重构即架构设计判据(回填适用)
> **原始执行**: Stage 6.5(TD-011 step 5) — 当時未产出独立 plan 文档,仅 gate review
> **回填日期**: 2026-07-26(Stage 12.9 backfill per r217 stages-5-8 audit §7 P2 item 6)

## 1. 目标(reconstructed from gate-review-6.5.md)

继续偿还 TD-011。第五步:将 field resolution(字段解析)相关函数从
`mir/lower/mod.rs` 提取到独立模块 `mir/lower/field_resolution.rs`。

依据 §14.4 "重构即架构设计",本步骤的架构判据聚焦于 J2(单一职责):
field resolution 是 MIR lowering 中独立的"HIR Place/Field → MIR Place"映射逻辑,
与 ADT layout(6.1)耦合点(都涉及 AdtLayout)但职责不同 — 6.1 构建 layout,
6.5 在 lowering 时查询 layout 以解析 field index。

## 2. MUV 拆分(reconstructed from gate-review-6.5.md "拆分结果" 表)

### 2.1 提取的函数

| 函数 | LOC(估) | 职责 |
|------|----------|------|
| field resolution 主函数(`lower_*_field` 系列) | ~150 | 将 HIR `HirExprKind::Field` 降低为 MIR `Place` + `Field` 投影 |
| field index 查询辅助(`adt_field_index` 等) | ~17 | 从 `AdtLayout` 查询 field name → field index |
| 相关 use / trait impl 周边 | ~0 | 共享 MirLowerCtxt,无新 trait |

(精确的逐函数清单在原始执行时未单独记录,本表由 gate-review-6.5.md "mir/lower/field_resolution.rs — 167 LOC 新建"反推;167 LOC 含模块头、use 块、AdtLayout 查询逻辑与 place projection 构造逻辑。)

### 2.2 §16 接口隔离

提取后 `mir/lower/field_resolution.rs` 依赖:
- `mir::body::*`(`MirBody`, `StatementKind`)
- `mir::place::*`(`Place`, `ProjectionElem`, `Local`)
- `mir::lower::adt_layout::*`(`AdtLayout`,从 6.1 提取的模块,`pub(crate)` 可见)
- `mir::lower::*`(`MirLowerCtxt`)
- `hir::*`(`HirExpr`, `HirExprKind::Field`, `Res`)

所有依赖单向(`mir::lower` 内部模块间,field_resolution → adt_layout),无循环。✅

### 2.3 命名标准化

无新公共 API — 仅内部模块重组。

## 3. §14.4 J1-J6 判据(reconstructed)

| 判据 | 评估 | GO/NO-GO |
|------|------|----------|
| J1 架构对齐 | field resolution 是 MIR lowering 的独立子阶段,与 overflow assert(6.4)/ control_flow(6.6)正交 | ✅ GO |
| J2 单一职责 | 新模块仅负责 HIR Field → MIR Place + 投影,职责单一 | ✅ GO |
| J3 单向流动 | 依赖 `adt_layout`(6.1)的 AdtLayout,产出 MIR `Place` 投影;无反向依赖 | ✅ GO |
| J4 编译表达完整 | 拆分后所有 rust types 仍可被现有 MIR 表达,无新 type 引入 | ✅ GO |
| J5 阶段划分 | 作为 TD-011 step 5,在 6.4(已完成)与 6.6(待执行)之间 | ✅ GO |
| J6 科学粒度 | 204 LOC 提取量适中,与前后步骤形成连续 7 步 TD-011 序列 | ✅ GO |

**6/6 GO**(回填判据,原始执行时 v3.20 未要求 J1-J6 表格,但拆分实际满足全部判据)。

## 4. 验收标准(reconstructed)

1. `cargo test`:**1881 passed, 0 failed, 2 ignored**(行为等价重构,0 新测试)
2. `cargo fmt --check`:clean(exit 0)
3. `cargo clippy --all-targets`:0 warnings, 0 errors
4. `mir/lower/mod.rs` LOC 减少 ≥ 200(实际 -204)
5. 新建 `mir/lower/field_resolution.rs` 模块,LOC ≤ 200(实际 167)
6. §16 依赖方向单向,field_resolution → adt_layout,无循环

## 5. 实际执行结果(from gate-review-6.5.md)

**CI/CD**(原始执行,2026-07-24):

```
cargo clean: clean (568.8 MiB removed)
cargo test: 1881 passed, 0 failed, 2 ignored
cargo fmt --check: clean (exit 0)
cargo clippy --all-targets: 0 warnings, 0 errors
```

**拆分结果**:

| 文件 | Before | After | 变化 |
|------|--------|-------|------|
| `mir/lower/mod.rs` | 2656 LOC | 2452 LOC | -204 LOC (-7.7%) |
| `mir/lower/field_resolution.rs` | — | 167 LOC | 新建 |

**TD-011 累计进度**(截至 6.5 完成):

| Split | Module | LOC extracted | mod.rs after |
|-------|--------|--------------|--------------|
| 6.1 | adt_layout.rs | 153 | 3193 |
| 6.2 | closure_capture.rs | 158 | 3035 |
| 6.3 | pattern_bindings.rs | 305 | 2730 |
| 6.4 | overflow_assert.rs | 74 | 2656 |
| 6.5 | field_resolution.rs | 204 | 2452 |
| **Total** | **5 modules** | **894 LOC** | **2452 (was 3346, -26.7%)** |

**Gate review verdict**: 5/5 GO → PASS

## 6. 关联文档

- `docs/develop/v0/stage-6/gate-review-6.5.md` — 原始 gate review(本 plan 的回填来源)
- `docs/develop/v0/stage-6/plan-6.1.md` ~ `plan-6.4.md` — 前序 plan(TD-011 step 1-4,参考格式)
- `docs/develop/v0/stage-6/plan-6.6.md` — 后续 plan(回填,TD-011 step 6)
- `docs/develop/v0/stage-12/cross-stage-audit-r217-stages-5-8.md` §3 + §7 P2 item 6 — 回填来源审计
- `docs/stage-committee-process.md` §14.4(重构即架构设计,J1-J6 判据)+ §17.3 时期 2(原始执行的流程版本)

---

**回填日期**: 2026-07-26 (Stage 12.9)
**原始执行日期**: 2026-07-24 (Stage 6.5, v0.12.3 → v0.12.4)
