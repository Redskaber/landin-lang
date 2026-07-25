# Stage 10 — Test Documentation

> **阶段范围**: Stage 10.0 - 10.8 (9 sub-stages planned)
> **测试目录**: `tests/v0/stage10/plan/` + `tests/conformance/01-07-*`
> **状态**: 🔄 In Progress (10.0-10.4 complete, 10.5 in progress)

## 测试目录结构

```
tests/v0/stage10/plan/
├── README.md                    ← 本文件
├── stage10_0_tests.rs           (8 tests, Stage 10.0 — CLI/runner upgrade)
├── stage10_1_tests.rs           (6 tests, Stage 10.1 — typecheck conformance)
├── stage10_2_tests.rs           (4 tests, Stage 10.2 — borrowck conformance)
├── stage10_3_tests.rs           (4 tests, Stage 10.3 — codegen conformance)
├── stage10_4_tests.rs           (4 tests, Stage 10.4 — e2e conformance)
└── stage10_5_tests.rs           (4 tests, Stage 10.5 — soundness conformance)

tests/conformance/               ← v0.1 conformance suite (target 5000)
├── 00-parse/                    (600 tests, Stage 9 — 100% ✅)
├── 01-typecheck/                (120 tests, Stage 10.1 — initial batch)
├── 02-borrowck/                 (80 tests, Stage 10.2 — initial batch)
├── 03-codegen/                  (61 tests, Stage 10.3 — initial batch)
├── 04-e2e/                      (48 tests, Stage 10.4 — initial batch)
├── 05-soundness/                (50 tests, Stage 10.5 — in progress)
├── 06-stdlib/                   (planned, Stage 10.6)
├── 07-integration/              (planned, Stage 10.7)
└── run_all.py                   (conformance runner with --mode auto)
```

## Conformance progress

| Category | Required | Current | % |
|----------|---------|---------|---|
| 00-parse | 600 | 600 | 100% ✅ |
| 01-typecheck | 1000 | 120 | 12% |
| 02-borrowck | 800 | 80 | 10% |
| 03-codegen | 600 | 61 | 10.2% |
| 04-e2e | 500 | 48 | 9.6% |
| 05-soundness | 500 | 0 → 50 | 0% → 10% |
| 06-stdlib | 500 | 0 | 0% |
| 07-integration | 500 | 0 | 0% |
| **Total** | **5000** | **909** | **18.2%** |
