# Stage 2 — Test Documentation

> **阶段范围**: Stage 2.1 - 2.5 (MIR lowering + typeck + borrowck foundations)
> **测试目录**: `tests/v0/stage2/plan/`
> **状态**: ✅ Complete

## 测试目录结构

```
tests/v0/stage2/plan/
├── README.md                    ← 本文件
├── mir_lowering_tests.rs        (Stage 2.1 — HIR → MIR lowering)
├── negative_cases_tests.rs      (Stage 2.2 — typeck/borrowck negative tests)
├── integration_tests.rs         (Stage 2.3 — MIR ↔ typeck integration)
└── typeck_tests.rs              (Stage 2.4 — type checker + borrow checker)
```

> Note: Per r217 second-pass audit (Stage 12.7 correction), actual filenames are
> `negative_cases_tests.rs`, `integration_tests.rs`, `typeck_tests.rs`
> (not `negative_cases.rs`, `integration.rs`, `typeck_borrowck_tests.rs` as previously listed).

## 测试统计

| Type | Count |
|------|-------|
| Rust integration tests | 141 |

## 测试覆盖

> Per-module counts verified by r217 second-pass audit (Stage 12.7 correction).
> Total = 141 ✅ (matches); per-module breakdown corrected from r216 first-pass estimates.

| Module | Tests | Focus |
|--------|-------|-------|
| mir_lowering_tests.rs | 22 | BasicBlock, LocalDecl, Terminator, Statement lowering |
| negative_cases_tests.rs | 35 | Type errors, borrow errors, expected compile_error |
| integration_tests.rs | 58 | MIR → typeck → borrowck data flow |
| typeck_tests.rs | 26 | Type inference, NLL borrow check |
| **Total** | **141** | (verified r217) |

## 关联文档

- `docs/develop/v0/stage-2/dev-log.md` — Stage 2 开发日志
- `docs/develop/v0/stage-2/gate-review-initial.md` — 初始门审查
- `docs/develop/v0/stage-2/gate-review-final.md` — 最终门审查
- `docs/develop/v0/stage-0-3-cross-stage-audit.md` — 跨阶段审查
- `docs/lang-design/06-mir.md` — MIR 设计
- `docs/tests/v0/stage2/plan/{mir_lowering,negative_cases,integration}.md` — 各模块测试设计文档

## 测试 runner

```bash
cargo test --test all_tests -- mir_lowering_tests negative_cases_tests integration_tests typeck_tests
```
