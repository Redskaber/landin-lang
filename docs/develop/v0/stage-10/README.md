# Stage 10 — v0.1 Conformance Suite 补全 (5000 tests)

> **阶段范围**: Stage 10.0 - 10.8 (9 sub-stages planned)
> **版本范围**: v0.17.0 → v0.20.x (target v0.1 release)
> **流程**: stage-committee-process.md v3.21 (§13.4 + §17.1/§17.2/§17.3 + §25 + §1.2)
> **状态**: 🔄 In Progress (10.0-10.4 complete, 10.5 in progress)

## 阶段目标

补全 v0.1 conformance suite 从 600 parse tests 到 5000 tests (8 categories per
`17-conformance-suite.md` §5.1), 真实达成 v0.1 release gate。

## 子阶段索引

| 子阶段 | 主题 | 测试数 | 累计 | 状态 |
|--------|------|-------|------|------|
| 10.0 | CLI upgrade + Runner upgrade | 0 (infra) | 600 | ✅ Complete |
| 10.1 | 01-typecheck conformance | +120 | 720 | ✅ Complete |
| 10.2 | 02-borrowck conformance | +80 | 800 | ✅ Complete |
| 10.3 | 03-codegen conformance | +61 | 861 | ✅ Complete |
| 10.4 | 04-e2e conformance | +48 | 909 | ✅ Complete |
| 10.5 | 05-soundness conformance | +50 | 959 | 🔄 In Progress |
| 10.6 | 06-stdlib conformance | +50 | 1009 | 🟡 Planned |
| 10.7 | 07-integration conformance | +50 | 1059 | 🟡 Planned |
| 10.8 | §25 deep review + v0.1 release | — | 5000 | 🟡 Planned |

## 关联文档

- `v0.1-gap-analysis.md` — v0.1 需求 vs 当前状态 gap 分析
- `gate-review-v0.1-gap.md` — gap 分析 gate review
- `plan-stage10.md` — Stage 10 总体计划
- `plan-10.{0..5}.md` — 各子阶段开发计划
- `gate-review-10.{0..5}.md` — 各子阶段门审查

## 关联测试

- `tests/v0/stage10/plan/stage10_{0..5}_tests.rs` — 各子阶段验证测试
