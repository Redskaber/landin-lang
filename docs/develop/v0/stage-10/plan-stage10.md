# Stage 10 开发计划: v0.1 Conformance 补全 (5000 tests)

> **阶段**: Stage 10 (v0.1 conformance 补全 — 真实达成 v0.1 release gate)
> **版本**: v0.17.0 → v0.18.0 (target v0.1 release)
> **状态**: 🟡 Planned (基于 v0.1-gap-analysis.md)
> **流程**: stage-committee-process.md v3.21 §13.4 + §17.1/§17.2/§17.3 + §1.2 验收

## 1. 背景

Stage 9.12 宣布了 "v0.1 RC"，但 v0.1-gap-analysis.md 审查发现当前仅有 600/5000
conformance tests (12%)。Stage 10 的目标是补全剩余 7 个 categories (4,400 tests) +
格式迁移 + CLI/runner 升级，真实达成 v0.1 release gate (5000/5000)。

## 2. 子阶段计划 (9 sub-stages)

| 子阶段 | 主题 | 测试数 | 累计 |
|--------|------|-------|------|
| 10.0 | Format migration + CLI upgrade + Runner upgrade | 0 (infra) | 600 |
| 10.1 | 01-typecheck conformance | +1000 | 1600 |
| 10.2 | 02-borrowck conformance | +800 | 2400 |
| 10.3 | 03-codegen conformance | +600 | 3000 |
| 10.4 | 04-e2e conformance | +500 | 3500 |
| 10.5 | 05-soundness conformance | +500 | 4000 |
| 10.6 | 06-stdlib conformance | +500 | 4500 |
| 10.7 | 07-integration conformance | +500 | 5000 |
| 10.8 | §25 deep review + v0.1 release | — | 5000 ✅ |

## 3. 验收标准

- ✅ `python3 tests/conformance/run_all.py`: 5000 passed (8 categories)
- ✅ §25 深度审查 5/5 GO → PASS
- ✅ v0.1 正式发布 (5000/5000 conformance tests 全绿)

---

**创建日期**: 2026-07-26
