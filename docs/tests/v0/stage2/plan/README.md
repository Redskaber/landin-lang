# Stage 2 — Test Documentation

> **阶段范围**: Stage 2.1 - 2.5 (MIR lowering + typeck + borrowck foundations)
> **测试目录**: `tests/v0/stage2/plan/`
> **状态**: ✅ Complete

## 测试目录结构

```
tests/v0/stage2/plan/
├── README.md                    ← 本文件
├── mir_lowering_tests.rs        (Stage 2.1 — HIR → MIR lowering)
├── negative_cases.rs            (Stage 2.2 — typeck/borrowck negative tests)
├── integration.rs               (Stage 2.3 — MIR ↔ typeck integration)
└── typeck_borrowck_tests.rs     (Stage 2.4 — type checker + borrow checker)
```

## 测试统计

| Type | Count |
|------|-------|
| Rust integration tests | 141 |

## 测试覆盖

| Module | Tests | Focus |
|--------|-------|-------|
| mir_lowering_tests.rs | 45 | BasicBlock, LocalDecl, Terminator, Statement lowering |
| negative_cases.rs | 30 | Type errors, borrow errors, expected compile_error |
| integration.rs | 35 | MIR → typeck → borrowck data flow |
| typeck_borrowck_tests.rs | 31 | Type inference, NLL borrow check |

## 关联文档

- `docs/develop/v0/stage-2/dev-log.md` — Stage 2 开发日志
- `docs/develop/v0/stage-2/gate-review-initial.md` — 初始门审查
- `docs/develop/v0/stage-2/gate-review-final.md` — 最终门审查
- `docs/develop/v0/stage-0-3-cross-stage-audit.md` — 跨阶段审查
- `docs/lang-design/06-mir.md` — MIR 设计
- `docs/tests/v0/stage2/plan/{mir_lowering,negative_cases,integration}.md` — 各模块测试设计文档

## 测试 runner

```bash
cargo test --test all_tests -- mir_lowering_tests negative_cases integration typeck_borrowck_tests
```
