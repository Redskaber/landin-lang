# Stage 18.275 — Process Doc v6.4 → v7.0 Optimization

> **Author**: Super Z (main) — Stage Committee (ARCH-A + PM-A + REV-A + QA-A)
> **Date**: 2026-08-25
> **Version**: v0.492.0 (no bump — process doc optimization)
> **Process**: stage-committee-process.md v6.4 §3.3 (Spec 持续演进)
> **Status**: ✅ Complete — v7.0 100% covers v6.4 + integrates 7 user directives

---

## 1. Executive Summary

Deep audit + optimization of `docs/stage-committee-process.md` from v6.4
(2935 LOC) to v7.0 (3116 LOC). Integrated user's 7 execution directives
at correct locations, added 3 new chapters (§18, §19, §20), enhanced
2 existing sections (§17.7, §17.8), added new §2.1.1 (每轮执行原则),
corrected §3.2 to `--release` mode, updated LLVM references from 19 to 22.

### 1.1 Outcomes

| Change | Location | Description |
|--------|----------|-------------|
| New §2.1.1 | 每轮执行原则 | 10 rules: 按计划推进, API命名, 接口设计, 通解>特解, 高内聚低耦合, 单一职责, 管道流, 层级划分, 避免死代码, 避免分散内容 |
| §3.2 fix | 验收命令 | Corrected to `--release` mode |
| §3.1 fix | LLVM references | Updated from LLVM 19 to LLVM 22 |
| §17.7 enhancement | 缺陷纳入 | Added "整体性修复" rule |
| §17.8 enhancement | 优化补充 | Added "任务阻塞审查" + "任务重排" |
| New §18 | 依赖与基础设施审查 | Trigger: when design depends on lower-level impl. Core: "直到审查不出问题为止" |
| New §19 | 阶段打包规则 | Every stage must be packaged as tar.gz |
| New §20 | 迭代审计原则 | After fixing bug, audit similar paths until convergence |
| §16 update | 变更日志 | Added v7.0 entry |
| 目录 update | Table of Contents | Added §18, §19, §20 |

### 1.2 v6.4 → v7.0 Coverage

**100% backward-compatible**: All v6.4 rules preserved, no deletions.
3 new chapters + 1 new section + 2 enhancements + 2 corrections.

---

## 2. User Directive Integration Mapping

| # | User Directive | Integrated At | Action |
|---|---------------|---------------|--------|
| 1 | 每轮根据审查报告...严格API命名标准化和接口设计 | §2.1.1 | New section with 10 execution rules |
| 2 | 明确设计与开发和测试原则...避免分散内容 | §2.1.1 rules 4-10 | Pipeline flow, layer division, avoid dead code, avoid scattered content |
| 3 | 如果当前设计和实现存在简写和缺陷或MVP...整体性完整修复 | §17.7 | Added "整体性修复" rule |
| 4 | 如果在设计和开发过程中设计的内容需要依赖底层实现...直到审查不出问题为止 | §18 (new) | New chapter with full audit protocol |
| 5 | 如果在开始选择处理任务时遇到任务依赖缺陷...重排任务排版图 | §17.8 | Added "任务阻塞审查" + "任务重排" |
| 6 | 校验流：cargo clean...--release... | §3.2 | Corrected to --release mode |
| 7 | 每一个stage阶段都压缩打包... | §19 (new) | New chapter with packaging rules |

---

## 3. Verification

- ✅ cargo fmt --check — 0 diff
- ✅ cargo check --features llvm-backend — 0 errors, 0 warnings
- ✅ cargo test --release --features llvm-backend — 3914 tests, 0 failures
- ✅ Process doc: 2935 → 3116 LOC (+181 LOC from new chapters)
- ✅ 100% backward-compatible with v6.4
