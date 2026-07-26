# Stage 13 — Development Documentation Index

> **阶段范围**: Stage 13.1 - 13.6 (v0.3 self-hosting preparation — compile pipeline fixes)
> **流程**: stage-committee-process.md v3.21 (§13.4 + §14.4 + §25 + §25.8)
> **状态**: 🔄 In Progress (13.1 — architecture baseline + audit/plan verification)
> **基线**: v0.21.0 (post Stage 12.1, v0.1 gate reached)
> **目标**: v0.22.0 (after Stage 13.1) → v0.23.0 (after 13.2-13.4) → v0.24.0 (after 13.5+)

## Stage 13 启动依据

Stage 13 启动基于 Stage 12 跨阶段审计 (r216) 的结论：

- v0.1 conformance gate 已达到 (5026/5000)
- v0.1 release 已准备就绪 (Stage 12.1 已 ratify)
- v0.3 自举前置条件未满足（3 个 P0 + 1 个 P1 阻塞项）
- 推荐执行 Option B (编译管线修复) 而非 Option A (发布公告)

完整审计文档：
- `docs/develop/v0/stage-12/cross-stage-audit-r216-architecture.md` (ARCH-A, D1+D5)
- `docs/develop/v0/stage-12/cross-stage-audit-r216-techdebt-tests-docs.md` (D2+D3+D4+D6+D7)

## 子阶段索引

| Sub-stage | Status | Plan | Gate Review | Dev Log |
|-----------|--------|------|-------------|---------|
| 13.1 | 🔄 In Progress | `plan-13.1.md` | `gate-review-13.1.md` (TBD) | (TBD) |
| 13.2 | ⏳ Planned | (TBD) | — | — |
| 13.3 | ⏳ Planned | (TBD) | — | — |
| 13.4 | ⏳ Planned | (TBD) | — | — |
| 13.5 | ⏳ Planned | (TBD) | — | — |
| 13.6 | ⏳ Planned | (TBD) | — | — |

## Stage 13 目标

1. **关闭 3 个 P0 阻塞项** (TD-030/031/032) — v0.3 自举硬前置
2. **关闭 2 个 P2 重构项** (TD-028/029) — §16 合规 + §25.8 回写
3. **执行 6 个 P1 子项** (TD-033.1-6) — 并行 Stage 1 起草
4. **Stage 13.6 公开发布 v0.1** — 公告 + tag + changelog finalize

## 关键文档

- `docs/develop/v0/stage-13/plan-13.1.md` — Stage 13.1 完整 plan (含 MUV 拆分 + §14.4 判据 + §13.4 设计对齐)
- `docs/develop/v0/stage-12/cross-stage-audit-r216-architecture.md` — ARCH-A D1+D5 审计
- `docs/develop/v0/stage-12/cross-stage-audit-r216-techdebt-tests-docs.md` — D2+D3+D4+D6+D7 审计
- `docs/develop/v0/stage-12/v0.3-bootstrap-prep.md` — v0.3 自举规划
- `docs/lang-design/13-stage1-feature-whitelist.md` — Stage 1 特性白皮书

## 关联测试

- `tests/v0/stage13/plan/stage13_1_tests.rs` — Stage 13.1 验证测试 (10 tests)
- `docs/tests/v0/stage13/plan/README.md` — Stage 13 测试文档

---

**创建日期**: 2026-07-26
**Process**: v3.21 (§13.4 + §14.4 + §25 + §25.8)
