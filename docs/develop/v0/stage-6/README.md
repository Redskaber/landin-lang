# Stage 6 — Architectural Refactoring (TD-011/017/022-026)

> **阶段范围**: Stage 6.1 - 6.18 (18 sub-stages)
> **版本范围**: v0.11.95 → v0.14.0
> **流程**: stage-committee-process.md v3.21 (§14.4 重构即架构设计 + §25.8 设计回写)
> **状态**: ✅ Complete

## 阶段目标

偿还 6 个架构性技术债，将 47 个模块按单一职责原则拆分，使所有大文件 < 1500 LOC。
重构本质是组织结构设计，不是单纯缩小文件体积——拆分依据 J1-J6 六大判据
(架构对齐 / 单一职责 / 单向流动 / 编译表达完整 / 阶段划分 / 科学粒度)。

## 子阶段索引

| 子阶段 | 主题 | 文件 |
|--------|------|------|
| 6.1 | mir/lower adt_layout split (TD-011 step 1) | plan-6.1.md, gate-review-6.1.md |
| 6.2 | mir/lower closure_capture split (TD-011 step 2) | plan-6.2.md, gate-review-6.2.md |
| 6.3 | mir/lower pattern_bindings split (TD-011 step 3) | plan-6.3.md, gate-review-6.3.md |
| 6.4 | mir/lower overflow_assert split (TD-011 step 4) | plan-6.4.md, gate-review-6.4.md |
| 6.5 | mir/lower field_resolution split (TD-011 step 5) | plan-6.5.md, gate-review-6.5.md |
| 6.6 | mir/lower control_flow split (TD-011 step 6) | plan-6.6.md, gate-review-6.6.md |
| 6.7 | codegen trait_dispatch split (TD-017 step 1) | plan-6.7.md, gate-review-6.7.md |
| 6.8 | codegen mir_translation split (TD-017 step 2) | plan-6.8.md, gate-review-6.8.md |
| 6.9 | stdlib 3-domain split | plan-6.9.md, gate-review-6.9.md |
| 6.10 | mir/lower expr_operand split (TD-011 step 7) | plan-6.10.md, gate-review-6.10.md |
| 6.11 | process v3.21 governance protocol | plan-6.11.md, gate-review-6.11.md |
| 6.12 | parser.rs split per §3.1-§3.7 (TD-022) | plan-6.12.md, gate-review-6.12.md |
| 6.13 | lexer/reader.rs split per §1 (TD-023) | plan-6.13.md, gate-review-6.13.md |
| 6.14 | borrowck/mod.rs split per §4 NLL (TD-024) | plan-6.14.md, gate-review-6.14.md |
| 6.15 | typeck/checker.rs split per §4+§8 (TD-025) | plan-6.15.md, gate-review-6.15.md |
| 6.16 | resolve/resolver.rs split per §6.2 (TD-026) | plan-6.16.md, gate-review-6.16.md |
| 6.17 | mir/lower expr_operand sub-split — REVERTED in 6.18 | plan-6.17.md, gate-review-6.17.md |
| 6.18 | Stage 6 finale: §25.8 design writeback + refactoring concluded | plan-6.18.md, gate-review-6.18.md |

## 关键里程碑

- 🎉 mod.rs below 2000 LOC (6.6)
- 🎉 Codegen 5-module architecture complete (6.8)
- 🎉 process v3.21 fully landed (6.11)
- 🎉 parser.rs 3112 → 263 LOC, -91.5% (6.12)
- 🎉 All major files < 1500 LOC (6.18)

## 技术债状态

| ID | 描述 | 状态 |
|----|------|------|
| TD-011 | mir/lower/mod.rs 拆分 | ✅ CLOSED |
| TD-017 | codegen/mod.rs 拆分 | ✅ CLOSED |
| TD-022 | parser.rs 拆分 | ✅ CLOSED |
| TD-023 | lexer/reader.rs 拆分 | ✅ CLOSED |
| TD-024 | borrowck/mod.rs 拆分 | ✅ CLOSED |
| TD-025 | typeck/checker.rs 拆分 | ✅ CLOSED |
| TD-026 | resolve/resolver.rs 拆分 | ✅ CLOSED |
| TD-019 | expr_operand 巨型 match | 🟡 OPEN (user-directed hold) |
