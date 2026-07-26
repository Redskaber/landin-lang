# Stage 6.4 开发计划(回填):mir/lower/mod.rs 拆分 — overflow_assert 提取(TD-011 第四步)

> **状态**: ✅ Complete (本 plan 于 Stage 12.9 从 `gate-review-6.4.md` 回填重建)
> **流程**: stage-committee-process.md v3.20 §17.3 时期 2(原始执行);v3.21 §14.4 重构即架构设计判据(回填适用)
> **原始执行**: Stage 6.4(TD-011 step 4) — 当時未产出独立 plan 文档,仅 gate review
> **回填日期**: 2026-07-26(Stage 12.9 backfill per r217 stages-5-8 audit §7 P2 item 6)

## 1. 目标(reconstructed from gate-review-6.4.md)

继续偿还 TD-011(`mir/lower/mod.rs` 3346 LOC 拆分)。第四步:将 overflow assert
相关函数从 `mir/lower/mod.rs` 提取到独立模块 `mir/lower/overflow_assert.rs`。

依据 §14.4 "重构即架构设计",本步骤的架构判据为 J1(架构对齐)/ J2(单一职责)/
J3(单向流动):overflow assert 是 MIR lowering 阶段独立的运行时检查注入逻辑,
与 ADT layout(6.1)/ closure capture(6.2)/ pattern bindings(6.3)正交,可独立成模块。

## 2. MUV 拆分(reconstructed from gate-review-6.4.md "拆分结果" 表)

### 2.1 提取的函数

| 函数 | LOC(估) | 职责 |
|------|----------|------|
| overflow assert 注入辅助函数 | 74 | 在算术运算 MIR lower 时注入 `OverflowAssert` statement,承载 `BinaryOp` 溢出检查 |

(精确的逐函数清单在原始执行时未单独记录,本表由 gate-review-6.4.md "mir/lower/overflow_assert.rs — 94 LOC 新建"反推;94 LOC 包含模块头/use 块/trait impl 等约 20 LOC 周边代码。)

### 2.2 §16 接口隔离

提取后 `mir/lower/overflow_assert.rs` 依赖:
- `mir::body::*`(`StatementKind`, `MirBody`)
- `mir::place::*`(`Rvalue`, `BinOp`)
- `mir::lower::*`(`MirLowerCtxt`, lowering 上下文)

所有依赖单向(`mir::lower` 内部模块间),无循环。✅

### 2.3 命名标准化

无新公共 API — 仅内部模块重组。相关函数从 `fn` 改为 `pub(crate) fn`(若需跨模块调用)。

## 3. §14.4 J1-J6 判据(reconstructed)

| 判据 | 评估 | GO/NO-GO |
|------|------|----------|
| J1 架构对齐 | overflow_assert 与 ADT layout / closure capture / pattern_bindings 正交,属独立 MIR lowering 子阶段 | ✅ GO |
| J2 单一职责 | 新模块仅负责 overflow assert 注入,职责单一 | ✅ GO |
| J3 单向流动 | 依赖 `mir::lower::*` 输入,产出 `MirBody` mutations,无反向依赖 | ✅ GO |
| J4 编译表达完整 | 拆分后所有 rust types 仍可被现有 `MirBody` 表达,无新 type 引入 | ✅ GO |
| J5 阶段划分 | 作为 TD-011 step 4,在 6.1-6.3(已完成)与 6.5-6.7(待执行)之间 | ✅ GO |
| J6 科学粒度 | 74 LOC 提取量小,但与前后步骤形成连续 7 步 TD-011 序列,粒度合理 | ✅ GO |

**6/6 GO**(回填判据,原始执行时 v3.20 未要求 J1-J6 表格,但拆分实际满足全部判据)。

## 4. 验收标准(reconstructed)

1. `cargo test`:**1881 passed, 0 failed, 2 ignored**(行为等价重构,0 新测试)
2. `cargo fmt --check`:clean(exit 0)
3. `cargo clippy --all-targets`:0 warnings, 0 errors
4. `mir/lower/mod.rs` LOC 减少 ≥ 70(实际 -74)
5. 新建 `mir/lower/overflow_assert.rs` 模块,LOC ≤ 100(实际 94)
6. §16 依赖方向单向,无循环

## 5. 实际执行结果(from gate-review-6.4.md)

**CI/CD**(原始执行,2026-07-24):

```
cargo clean: clean (635.1 MiB removed)
cargo test: 1881 passed, 0 failed, 2 ignored
cargo fmt --check: clean (exit 0)
cargo clippy --all-targets: 0 warnings, 0 errors
```

**拆分结果**:

| 文件 | Before | After | 变化 |
|------|--------|-------|------|
| `mir/lower/mod.rs` | 2730 LOC | 2656 LOC | -74 LOC (-2.7%) |
| `mir/lower/overflow_assert.rs` | — | 94 LOC | 新建 |

**TD-011 累计进度**(截至 6.4 完成):

| Split | Module | LOC extracted | mod.rs after |
|-------|--------|--------------|--------------|
| 6.1 | adt_layout.rs | 153 | 3193 |
| 6.2 | closure_capture.rs | 158 | 3035 |
| 6.3 | pattern_bindings.rs | 305 | 2730 |
| 6.4 | overflow_assert.rs | 74 | 2656 |
| **Total** | **4 modules** | **690 LOC** | **2656 (was 3346, -20.6%)** |

**Gate review verdict**: 5/5 GO → PASS

## 6. 关联文档

- `docs/develop/v0/stage-6/gate-review-6.4.md` — 原始 gate review(本 plan 的回填来源)
- `docs/develop/v0/stage-6/plan-6.1.md` / `plan-6.2.md` / `plan-6.3.md` — 前序 plan(TD-011 step 1-3,参考格式)
- `docs/develop/v0/stage-6/plan-6.5.md` / `plan-6.6.md` — 后续 plan(回填,TD-011 step 5-6)
- `docs/develop/v0/stage-12/cross-stage-audit-r217-stages-5-8.md` §3 + §7 P2 item 6 — 回填来源审计
- `docs/stage-committee-process.md` §14.4(重构即架构设计,J1-J6 判据)+ §17.3 时期 2(原始执行的流程版本)

---

**回填日期**: 2026-07-26 (Stage 12.9)
**原始执行日期**: 2026-07-24 (Stage 6.4, v0.12.2 → v0.12.3)
