# Stage 8 — v0.2 Roadmap (Lifetime Elision + Object Safety + extern "C" + Drop + async/await)

> **阶段范围**: Stage 8.1 - 8.7 (7 sub-stages)
> **版本范围**: v0.14.9 → v0.15.6
> **流程**: stage-committee-process.md v3.21 (§13.4 + §14.4 + §25 + §25.8 + §17.1)
> **状态**: ✅ Complete

## 阶段目标

完成 v0.2 路线图全部 5 项特性 + §25.8 设计回写 + §25 深度审查 + §17.1 文档目录标准化。

## 子阶段索引

| 子阶段 | 主题 | 文件 |
|--------|------|------|
| 8.1 | Lifetime elision (§3.2 RFC #141) | plan-8.1.md, gate-review-8.1.md |
| 8.2 | Object safety (§2.3 RFC #255) | plan-8.2.md, gate-review-8.2.md |
| 8.3 | extern "C" ABI (§13.2) | plan-8.3.md, gate-review-8.3.md |
| 8.4 | Drop elaboration (§5) | plan-8.4.md, gate-review-8.4.md |
| 8.5 | async/await foundation (§10) | plan-8.5.md, gate-review-8.5.md |
| 8.6 | §25.8 design writeback + §25 deep review GO | gate-review-8.6.md, deep-review-stage8-r181.md |
| 8.7 | Docs reorganization + worklog sync + §17.1 tests dir 标准 | plan-8.7.md, gate-review-8.7.md |

## v0.2 路线图状态

| 优先级 | 特性 | RFC/章节 | 状态 |
|--------|------|---------|------|
| P1 | Lifetime elision | §3.2 RFC #141 | ✅ Stage 8.1 |
| P2 | Object safety | §2.3 RFC #255 | ✅ Stage 8.2 |
| P2 | extern "C" ABI | §13.2 | ✅ Stage 8.3 |
| P2 | Drop elaboration | §5 | ✅ Stage 8.4 |
| P3 | async/await | §10 | ✅ Stage 8.5 (MVP synchronous) |

**🎉 v0.2 路线图全部 5 项特性完成！**

## 关键里程碑

- 🎉 v0.2 P1 lifetime elision COMPLETE (8.1)
- 🎉 v0.2 P2 object safety COMPLETE (8.2)
- 🎉 v0.2 P2 extern "C" ABI COMPLETE (8.3)
- 🎉 v0.2 P2 drop elaboration COMPLETE (8.4)
- 🎉 v0.2 P3 async/await COMPLETE (8.5, MVP synchronous)
- 🎉 §25 deep review 5/5 GO → PASS (8.6, r181)
- 🎉 §25.8 design writeback to 4 docs (8.6, r182)
- 🎉 §17.1 docs/tests/v0/stage{6,7,8}/ standardized (8.7)

## 技术债状态

| ID | 描述 | 状态 |
|----|------|------|
| TD-019 | expr_operand 巨型 match | 🟡 OPEN (user-directed hold) |

## §25.8 设计回写 (8.6)

- `03-type-system.md` +§12 — 5 v0.2 features status update
- `04-ownership-borrowing.md` +§13 — lifetime elision + drop elaboration status
- `05-ast.md` +§14 — Await/Async expression variants 补写 (B4)
- `07-codegen.md` +§15 — extern "C" ABI status update

## §17.1 文档标准化 (8.7)

Stage 8.7 修复了 §17.1 / §17.2 / §18.4 长期存在的偏差：
- 创建 `docs/develop/v0/stage-6/`, `stage-7/`, `stage-8/` 目录
- 创建 `docs/tests/v0/stage6/`, `stage7/`, `stage8/` 目录
- 创建 `tests/v0/stage6/plan/` 目录 (Stage 6 was pure refactoring, no new tests)
- 移动 64 个 misplaced docs 从 `stage-5/` 到对应 stage 目录
- 同步 worklog.md (补全 stage6.10-r158 → stage8.6-r182 共 24 个 Task ID 条目)
- 补建 `plan-8.6.md` (之前只有 gate-review-8.6.md)

## 关联测试

- `tests/v0/stage8/plan/lifetime_elision_tests.rs` (7 tests)
- `tests/v0/stage8/plan/object_safety_tests.rs` (5 tests)
- `tests/v0/stage8/plan/extern_c_abi_tests.rs` (5 tests)
- `tests/v0/stage8/plan/drop_elaboration_tests.rs` (7 tests)
- `tests/v0/stage8/plan/async_await_tests.rs` (5 tests)
- `tests/v0/stage8/plan/deep_review_tests.rs` (9 tests)
