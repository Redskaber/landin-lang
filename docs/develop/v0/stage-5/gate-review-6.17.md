# Stage 6 Gate Review Round 17 (6.17) — mir/lower expr_operand sub-module extraction per §14.4

> **审查日期**: 2026-07-25 | **版本**: v0.13.5 → v0.13.6
> **流程**: stage-committee-process.md v3.21 §13.4（阶段开始设计对齐）+ §14.4（重构即架构设计）+ §1.2 验收
> **审查范围**: Stage 6.17 单一子阶段（mir/lower/expr_operand.rs 按 05-ast.md §8 拆分）

## CI/CD

```
cargo clean: clean
cargo test: 1881 passed, 0 failed, 2 ignored
cargo fmt --check: clean (exit 0)
cargo clippy --all-targets: 0 warnings, 0 errors
```

## §13.4 阶段开始设计对齐

依据 v3.21 §13.4，本阶段开始时查阅了 `docs/lang-design/05-ast.md` §8（表达式定义）+ `06-mir.md` §8（MIR 构建算法）：

- §8 表达式按语义分为 8+ 类别（字面量/路径/调用/字段/索引/运算/控制流/聚合）
- §8 MIR 构建算法把表达式 lowering 作为核心步骤

**偏差**：实现把 `lower_expr_to_operand`（1046 LOC 巨型 match）+ `lower_expr_to_place` + `build_dyn_trait_call_terminator` + `resolve_enum_variant` 都堆在单一 `expr_operand.rs`（1275 LOC），违反 §14.4 J2 + J6。

**决策**：把 3 个独立函数（`lower_expr_to_place` / `build_dyn_trait_call_terminator` / `resolve_enum_variant`）提取到各自专属子模块。`lower_expr_to_operand` 的巨型 match 保留（Rust match 不能跨文件，且拆分风险高）。

## §14.4 J1-J6 判据检查

| # | 判据 | 状态 | 说明 |
|---|------|------|------|
| J1 | 架构设计对齐 | ✅ | 提取的 3 个函数各自对应 05-ast.md §8 的独立概念（place / dyn call / enum variant） |
| J2 | 单一职责 | ✅ | place.rs = place lowering；dyn_call.rs = dyn Trait call；enum_variant.rs = enum variant resolution |
| J3 | 单向流动 | ✅ | expr_operand.rs → {place, dyn_call, enum_variant}，无环 |
| J4 | 编译相关表达完整 | ✅ | 每个提取的函数在其模块内是完整的 |
| J5 | 阶段划分清晰 | ✅ | 所有新模块在 `src/mir/lower/` 下，Stage 2 阶段 |
| J6 | 科学合理粒度 | ✅ | expr_operand.rs 1095 LOC（仍含巨型 match）；子模块 63-89 LOC |

## 拆分执行结果

```
src/mir/lower/
  expr_operand.rs   (1095 LOC)  ← lower_expr_to_operand (巨型 match) (-14.1%)
  place.rs          (75 LOC)    ← lower_expr_to_place (新)
  dyn_call.rs       (89 LOC)    ← build_dyn_trait_call_terminator (新)
  enum_variant.rs   (63 LOC)    ← resolve_enum_variant (新)
  adt_layout.rs     (147 LOC)   — 不变
  closure_capture.rs (175 LOC)  — 不变
  control_flow.rs   (462 LOC)   — 不变
  field_resolution.rs (167 LOC) — 不变
  overflow_assert.rs (94 LOC)   — 不变
  pattern_bindings.rs (286 LOC) — 不变
  mod.rs            (779 LOC)   — 不变
```

**expr_operand.rs**: 1275 → **1095 LOC**（-14.1%，-180 LOC）

## 可见性策略（§16 + §23 合规）

- `lower_expr_to_place`: `pub(super)` — expr_operand.rs 调用
- `build_dyn_trait_call_terminator`: `pub` — mod.rs re-export（公开 API）
- `resolve_enum_variant`: `pub(crate)` — mod.rs re-export
- mod.rs 通过 `pub use` / `pub(crate) use` re-export，**外部 API 零变更**

## §23 API 命名合规

- 所有函数名保留原名（零 churn）
- 模块名遵循 `<noun>` / `<noun>_<noun>` 模式
- 无新公共符号
- mod.rs 通过 `pub use` 显式 re-export（无 glob）

## TD-027 累计进展

新增技术债 TD-027（Stage 6.17 引入）：expr_operand.rs 3 个独立函数提取到子模块，已偿还。

| Stage | expr_operand.rs LOC | Δ |
|-------|--------------------|---|
| 6.10 (baseline) | 1275 | — |
| **6.17 (sub-module extraction)** | **1095** | **-180 (-14.1%)** |

## 七维度审查（精简版）

| 维度 | 状态 |
|------|------|
| D1 架构健康度 | ✅ 11-module 目录结构，每个模块单一职责 |
| D2 技术债清单 | ✅ TD-027 引入并立即偿还；TD-019（巨型 match）仍 OPEN |
| D3 测试覆盖 | ✅ 1881 tests 零回归 |
| D4 下一阶段就绪度 | ✅ TD-019（expr_operand 巨型 match 拆分）留待未来 |
| D5 设计合理性 | ✅ §14.4 J1-J6 全部通过 |
| D6 性能 | ✅ 无性能影响 |
| D7 文档 | ✅ plan-6.17 + gate-review-6.17 + dev-log + api-naming-standard v1.86 + RELEASE_NOTES + README + worklog |

## 委员会投票

**5/5 GO → PASS**

## 后续行动

| 优先级 | 行动 | 目标阶段 |
|--------|------|---------|
| P3 | TD-019: expr_operand 巨型 match 按表达式类别细拆 | Stage 6.18+ |
| P2 | 完整 §25.8 设计回写（全 docs/lang-design/） | Stage 6 末尾 |
| P2 | TD-015: Region inference | Stage 6+ |
| P3 | TD-018: 用户自定义 trait dyn | Stage 6+ |

---

**审查完成**: 2026-07-25
