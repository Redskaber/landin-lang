# Stage 8 — Test Documentation

> **阶段范围**: Stage 8.1 - 8.7 (7 sub-stages)
> **测试目录**: `tests/v0/stage8/plan/`
> **测试总数**: 38 new tests (Stage 8 added)
> **状态**: ✅ Complete

## 测试目录结构

```
tests/v0/stage8/
└── plan/
    ├── lifetime_elision_tests.rs      (7 tests, Stage 8.1)
    ├── object_safety_tests.rs         (5 tests, Stage 8.2)
    ├── extern_c_abi_tests.rs          (5 tests, Stage 8.3)
    ├── drop_elaboration_tests.rs      (7 tests, Stage 8.4)
    ├── async_await_tests.rs           (5 tests, Stage 8.5)
    └── deep_review_tests.rs           (9 tests, Stage 8.6)
```

## 测试矩阵

| 子阶段 | 测试文件 | 测试数 | 目的 |
|--------|---------|-------|------|
| 8.1 | lifetime_elision_tests.rs | 7 | §3.2 RFC #141 lifetime elision rules |
| 8.2 | object_safety_tests.rs | 5 | §2.3 RFC #255 object safety rules |
| 8.3 | extern_c_abi_tests.rs | 5 | §13.2 extern "C" ABI support |
| 8.4 | drop_elaboration_tests.rs | 7 | §5 drop elaboration + drop order |
| 8.5 | async_await_tests.rs | 5 | §10 async/await MVP (synchronous) |
| 8.6 | deep_review_tests.rs | 9 | §25 deep review D1-D7 dimensions |
| 8.7 | (no new tests — docs reorganization) | 0 | §17.1/§17.2/§18.4 docs standardization |

**累计**: 2042 → 2100 (+58 tests across Stage 8, +2.8%)

## v0.2 路线图测试覆盖

| v0.2 特性 | RFC/章节 | 测试文件 | 测试数 |
|----------|---------|---------|-------|
| Lifetime elision | §3.2 RFC #141 | lifetime_elision_tests.rs | 7 |
| Object safety | §2.3 RFC #255 | object_safety_tests.rs | 5 |
| extern "C" ABI | §13.2 | extern_c_abi_tests.rs | 5 |
| Drop elaboration | §5 | drop_elaboration_tests.rs | 7 |
| async/await | §10 | async_await_tests.rs | 5 |

## 关联文档

- `docs/develop/v0/stage-8/README.md` — Stage 8 开发文档索引
- `docs/develop/v0/stage-8/plan-8.{1..7}.md` — 各子阶段开发计划
- `docs/develop/v0/stage-8/gate-review-8.{1..7}.md` — 各子阶段门审查
- `docs/develop/v0/stage-8/deep-review-stage8-r181.md` — §25 深度审查报告
