# Stage 9 — Test Documentation

> **阶段范围**: Stage 9.1 - 9.12 (12 sub-stages planned)
> **测试目录**: `tests/v0/stage9/plan/` + `tests/conformance/00-parse/`
> **状态**: 🟡 In Progress (Stage 9.1 complete)

## 测试目录结构

```
tests/v0/stage9/
└── plan/
    ├── README.md  ← 本文件
    └── systematic_review_v0156_tests.rs  (11 tests, Stage 9.1)

tests/conformance/
├── README.md
├── run_all.py
└── 00-parse/
    ├── 00-literals/   (33 .lin files after Stage 9.1; target: 100+)
    ├── 01-operators/  (target: 60+ tests, Stage 9.2)
    ├── 02-control-flow/  (1 + target: 80, Stage 9.3)
    ├── 03-patterns/  (1 + target: 70, Stage 9.4)
    ├── 04-types/  (target: 60, Stage 9.5)
    ├── 05-attributes/  (target: 40, Stage 9.6)
    ├── 06-generics/  (target: 50, Stage 9.7)
    ├── 07-closures/  (target: 40, Stage 9.8)
    ├── 08-modules/  (target: 60, Stage 9.9)
    ├── 09-error-recovery/  (1 + target: 50, Stage 9.10)
    └── 10-realistic/  (2 + target: 52, Stage 9.11)
```

## 测试矩阵

| 子阶段 | Rust 测试 | Conformance .lin | 累计 conformance |
|--------|----------|------------------|------------------|
| 9.1 | 11 (systematic_review_v0156_tests.rs) | +30 (literals) | 38 |
| 9.2 | TBD | +60 (operators) | 98 |
| 9.3 | TBD | +80 (control flow) | 178 |
| 9.4 | TBD | +70 (patterns) | 248 |
| 9.5 | TBD | +60 (types) | 308 |
| 9.6 | TBD | +40 (attributes) | 348 |
| 9.7 | TBD | +50 (generics) | 398 |
| 9.8 | TBD | +40 (closures) | 438 |
| 9.9 | TBD | +60 (modules) | 498 |
| 9.10 | TBD | +50 (error recovery) | 548 |
| 9.11 | TBD | +52 (realistic) | 600 |
| 9.12 | TBD (deep review) | — | 600 |

## 测试计划文档

- [x] `systematic_review_v0156.md` — Stage 9.1 systematic review + literals expansion
- [ ] `operators.md` — TODO Stage 9.2
- [ ] `control_flow.md` — TODO Stage 9.3
- [ ] `patterns.md` — TODO Stage 9.4
- [ ] `types.md` — TODO Stage 9.5
- [ ] `attributes.md` — TODO Stage 9.6
- [ ] `generics.md` — TODO Stage 9.7
- [ ] `closures.md` — TODO Stage 9.8
- [ ] `modules.md` — TODO Stage 9.9
- [ ] `error_recovery.md` — TODO Stage 9.10
- [ ] `realistic_programs.md` — TODO Stage 9.11
- [ ] `deep_review.md` — TODO Stage 9.12

## 关联文档

- `docs/develop/v0/stage-9/README.md` — Stage 9 开发文档索引
- `docs/develop/v0/stage-9/plan-9.{1..12}.md` — 各子阶段开发计划
- `docs/develop/v0/stage-9/gate-review-9.{1..12}.md` — 各子阶段门审查
- `docs/develop/v0/stage-9/systematic-review-v0156.md` — §25 系统性审查报告
- `docs/lang-design/17-conformance-suite.md` — conformance 套件设计规范
