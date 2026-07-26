# Stage 5 — TraitResolver + vtable + dyn Trait + stdlib 完整生态

> **阶段范围**: Stage 5.1 - 5.99 (99 distinct sub-stages: 96 documented plans + 3 deep-review-only milestones)
> **版本范围**: v0.11.0 → v0.11.95 (Stage 5 span; Stage 6 接续 v0.11.95 → v0.14.0)
> **流程**: stage-committee-process.md v3.20 → v3.21 (Stage 5 ran primarily on v3.20; v3.21 applied from Stage 5.82+ / Stage 6.11)
> **状态**: ✅ Complete

## 阶段目标

实现 TraitResolver + vtable + dyn Trait + stdlib 完整生态,为 Stage 6 的架构性重构
(TD-011/017/022-026) 与 Stage 7 的 region inference (TD-015) + user-defined trait dyn
(TD-018) 奠定基础。Stage 5 是项目最大的阶段(99 子阶段,977 rust 测试,502 conformance),
也是 dyn Trait MIR 4 层架构与 stdlib 5 类 43 trait 语义分组的设计源头。

## 子阶段索引

| 子阶段范围 | 主题 | 子阶段数 | 关键文件 |
|-----------|------|---------|---------|
| 5.1 - 5.20 | TraitResolver 基础 (collect trait defs + impls + dispatch tables) | 20 | plan-5.1.md ~ plan-5.20.md, gate-review-round1.md ~ round20.md |
| 5.21 | Deep Review #1 (§25, v0.11.20) — 7-Dimension Analysis | 1 | deep-review-r70.md (无独立 plan/gate-review 文件) |
| 5.22 - 5.26 | vtable layout 起步 + stdlib trait 探针 | 5 | plan-5.22.md ~ plan-5.26.md, gate-review-round22.md ~ round26.md |
| 5.27 | Deep Review #2 (§25, v0.11.26) | 1 | deep-review-r76.md (无独立 plan/gate-review 文件) |
| 5.28 - 5.31 | vtable emission + trait method dispatch | 4 | plan-5.28.md ~ plan-5.31.md, gate-review-round28.md ~ round31.md |
| 5.32 | Deep Review #3 (§25, v0.11.31) | 1 | deep-review-r81.md (无独立 plan/gate-review 文件) |
| 5.33 - 5.40 | vtable byte size + dyn Trait 前置 | 8 | plan-5.33.md ~ plan-5.40.md, gate-review-round33.md ~ round40.md |
| 5.41 - 5.60 | dyn Trait fat pointer + method call 基础 | 20 | plan-5.41.md ~ plan-5.60.md, gate-review-round41.md ~ round60.md |
| 5.61 - 5.80 | dyn Trait MIR 4 层架构 (DynTraitFatPtr → DynTraitMethodCall → DynTraitMIRSummary → DynTraitMIRPlan) | 20 | plan-5.61.md ~ plan-5.80.md, gate-review-round61.md ~ round80.md |
| 5.81 - 5.90 | stdlib trait methods 字段访问器 + 反向查询 (TD-016 closure at 5.82; TD-018 introduced at 5.90) | 10 | plan-5.81.md ~ plan-5.90.md, gate-review-round81.md ~ round90.md, deep-review-r91.md / r100.md / r110.md |
| 5.91 - 5.96 | stdlib trait methods 反向查询收尾 + stdlib semantic grouping (5 categories, 43 traits) | 6 | plan-5.91.md ~ plan-5.96.md, gate-review-round91.md ~ round96.md, deep-review-r120.md |
| 5.97 | Deep Review #7 (§25, v0.11.93) — Stage 5 最终深度审查 | 1 | deep-review-r120.md |
| 5.98 - 5.99 | stdlib_trait_methods_by_is_unsafe / by_param_count reverse queries — Stage 5 最终子阶段 | 2 | plan-5.98.md, plan-5.99.md, gate-review-round98.md, gate-review-round99.md |

**总计**: 99 distinct sub-stages(99 个不同子阶段),96 plan files + 96 gate-review files + 7 deep-review files = 200 entries(含 dev-log.md)。3 个 deep-review-only milestones(5.21 / 5.27 / 5.32)无独立 plan/gate-review 文件,详见 r217 §2.1 验证。

## 关键里程碑

- 🎉 TraitResolver 基础设施完成 (5.1-5.20, 20 子阶段)
- 🎉 vtable emission + byte size 完整 (5.33-5.40, v0.11.32 → v0.11.39)
- 🎉 dyn Trait fat pointer + method call 可用 (5.41-5.60, 20 子阶段)
- 🎉 dyn Trait MIR 4 层架构完成 (5.61-5.80, 20 子阶段)
- 🎉 TD-016 (dyn Trait return type I32 placeholder) CLOSED at 5.82 — 引入 StdlibTypeKind + stdlib_type_kind_to_emit_type() 转换器
- 🎉 stdlib semantic grouping: 5 categories / 43 traits (5.87-5.90)
- 🎉 Deep Review #7 GO → Stage 5 PASS (5.97, v0.11.93)
- 🎉 Stage 5 最终子阶段 5.99 完成,v0.11.95 → 接续 Stage 6.1 (v0.11.95)

## 技术债状态

| ID | 描述 | 状态 |
|----|------|------|
| TD-014 | L5 trait dispatch vtable | ✅ CLOSED |
| TD-016 | dyn Trait return type I32 placeholder | ✅ CLOSED at 5.82 (引入 StdlibTypeKind) |
| TD-018 | user-defined trait dyn | ✅ COMPLETE at Stage 7.6 (continued from Stage 5 foundation) |

## §25.8 状态

⚠️ Stage 5 ran primarily on v3.20 process — NO §25.8 design write-back was performed during Stage 5 (§25.8 protocol introduced in v3.21 at Stage 6.11, after Stage 5 concluded).

Stage 12.4 retroactively backfilled 3 implicit-knowledge items from Stage 5 deep reviews (per r217 §2.4 audit):

1. **`DynTraitMIRSummary` (Stage 5.71)** — dyn Trait MIR 4 层架构的第 3 层(项目汇总,介于 `DynTraitMethodCall` 与 `DynTraitMIRPlan` 之间)。回填到 `docs/lang-design/06-mir.md` §15。
2. **`StdlibTypeKind` + `stdlib_type_kind_to_emit_type()` (Stage 5.82, TD-016 closure)** — 将 stdlib type kinds 映射为 `EmitType` 的转换器。回填到 `docs/lang-design/09-stdlib.md` §12。
3. **stdlib semantic grouping (5 categories, 43 traits)** — 已在 `docs/lang-design/09-stdlib.md:1018` 中 ✓ (无 gap,无需回填)。

## 关联测试

- `tests/v0/stage5/plan/` — **92 .rs files, 977 #[test] items** (verified by r217 §2.2)
  - Top contributing files: `is_stdlib_trait_tests.rs` (24), `stdlib_trait_method_tests.rs` (24),
    `dyn_trait_return_kind_tests.rs` (23), `stdlib_core_traits_tests.rs` (22),
    `stdlib_vtable_layout_tests.rs` (22), `stdlib_io_unary_traits_tests.rs` (21),
    `stdlib_arithmetic_traits_tests.rs` (20), `stdlib_vtable_size_tests.rs` (20).
- `tests/conformance/06-stdlib/` — **502 .lin files** (100.4% of 500 target, verified by r217 §2.3)

## 关联文档

- `docs/develop/v0/stage-5/dev-log.md` — Stage 5 完整开发日志(99 子阶段)
- `docs/develop/v0/stage-5/deep-review-r{70,76,81,91,100,110,120}.md` — 7 个 deep review(r70=Stage 5.21, r76=Stage 5.27, r81=Stage 5.32, r91=Stage 5.41 区间, r100=Stage 5.81 区间, r110=Stage 5.91 区间, r120=Stage 5.97)
- `docs/lang-design/03-type-system.md` §2 (Trait system) + §13 (Stage 12 §25.8 write-back)
- `docs/lang-design/06-mir.md` §15 (Stage 12.4 §25.8 retroactive — DynTraitMIRSummary 4 层架构)
- `docs/lang-design/09-stdlib.md` §12 (Stage 12.4 §25.8 retroactive — StdlibTypeKind + stdlib_type_kind_to_emit_type()) + §1018 (semantic grouping ✓)
- `docs/develop/v0/stage-12/cross-stage-audit-r217-stages-5-8.md` §2 — Stage 5 re-audit(本 README 的数据来源)

---

**创建日期**: 2026-07-26 (Stage 12.9 backfill per r217 §7 P2 item 5)
**Process**: v3.21 (backfill applied retroactively per r217 second-pass audit; Stage 5 原始执行于 v3.20)
