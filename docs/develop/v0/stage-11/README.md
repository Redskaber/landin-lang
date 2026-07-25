# Stage 11 — v0.1 Conformance Per-Category Expansion

> **阶段范围**: Stage 11.1 - 11.8 (8 sub-stages planned)
> **版本范围**: v0.18.0 → v0.20.x (target v0.1 release)
> **流程**: stage-committee-process.md v3.21 (§13.4 + §17.1/§17.2/§17.3 + §25 + §1.2)
> **状态**: 🔄 In Progress (11.1 in progress)

## 阶段目标

逐个 category 扩展 conformance tests 到 §5.1 target 数量, 从 1139 → 5000.

## 子阶段计划

| 子阶段 | 主题 | 当前→目标 | 增量 | 累计 |
|--------|------|----------|------|------|
| 11.1 | 01-typecheck expansion | 200→400 | +200 | 1339 |
| 11.2 | 02-borrowck expansion | 80→300 | +220 | 1559 |
| 11.3 | 03-codegen expansion | 61→250 | +189 | 1748 |
| 11.4 | 04-e2e expansion | 48→200 | +152 | 1900 |
| 11.5 | 05-soundness expansion | 50→200 | +150 | 2050 |
| 11.6 | 06-stdlib expansion | 50→200 | +150 | 2200 |
| 11.7 | 07-integration expansion | 50→200 | +150 | 2350 |
| 11.8 | Final expansion + §25 deep review + v0.1 | →5000 | +2650 | 5000 |

## 关联测试

- `tests/v0/stage11/plan/stage11_{1..8}_tests.rs` — 各子阶段验证测试
