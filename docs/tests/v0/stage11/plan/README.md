# Stage 11 — Test Documentation

> **阶段范围**: Stage 11.1 - 11.10 (10 sub-stages)
> **测试目录**: `tests/v0/stage11/plan/` + `tests/conformance/01-07-*`
> **状态**: ✅ Complete — v0.1 conformance gate reached (5026/5000, 100.5%)

## 测试目录结构

```
tests/v0/stage11/plan/
├── README.md                    ← 本文件
├── stage11_1_tests.rs           (Stage 11.1 — typecheck expansion 200→400)
├── stage11_2_tests.rs           (Stage 11.2 — borrowck expansion 200→400)
├── stage11_3_tests.rs           (Stage 11.3 — codegen expansion 100→300)
├── stage11_4_tests.rs           (Stage 11.4 — e2e expansion 100→300)
├── stage11_5_tests.rs           (Stage 11.5 — soundness expansion 100→300)
├── stage11_6_7_tests.rs         (Stage 11.6 + 11.7 — stdlib + integration expansion)
├── stage11_8_tests.rs           (Stage 11.8 — batch expansion across all categories)
├── stage11_9_tests.rs           (Stage 11.9 — final push to v0.1 gate)
└── stage11_10_tests.rs          (Stage 11.10 — §25 deep review + v0.1 release prep)

tests/conformance/               ← v0.1 conformance suite (target 5000)
├── 00-parse/                    (600 tests — 100% ✅)
├── 01-typecheck/                (1020 tests — 102% ✅)
├── 02-borrowck/                 (800 tests — 100% ✅)
├── 03-codegen/                  (601 tests — 100.2% ✅)
├── 04-e2e/                      (502 tests — 100.4% ✅)
├── 05-soundness/                (500 tests — 100% ✅)
├── 06-stdlib/                   (502 tests — 100.4% ✅)
├── 07-integration/              (501 tests — 100.2% ✅)
└── run_all.py                   (conformance runner with --mode auto)
```

## Conformance final tally (v0.1 gate reached)

| Category | Required | Current | % |
|----------|---------|---------|---|
| 00-parse | 600 | 600 | 100% ✅ |
| 01-typecheck | 1000 | 1020 | 102% ✅ |
| 02-borrowck | 800 | 800 | 100% ✅ |
| 03-codegen | 600 | 601 | 100.2% ✅ |
| 04-e2e | 500 | 502 | 100.4% ✅ |
| 05-soundness | 500 | 500 | 100% ✅ |
| 06-stdlib | 500 | 502 | 100.4% ✅ |
| 07-integration | 500 | 501 | 100.2% ✅ |
| **Total** | **5000** | **5026** | **100.5% ✅** |

## Sub-stage overview

| Sub-stage | Focus | Conformance Δ | Result |
|-----------|-------|---------------|--------|
| 11.1 | typecheck expansion (200→400) | +200 | ✅ |
| 11.2 | borrowck expansion (200→400) | +200 | ✅ |
| 11.3 | codegen expansion (100→300) | +200 | ✅ |
| 11.4 | e2e expansion (100→300) | +200 | ✅ |
| 11.5 | soundness expansion (100→300) | +200 | ✅ |
| 11.6 | stdlib expansion (100→300) | +200 | ✅ |
| 11.7 | integration expansion (100→300) | +200 | ✅ |
| 11.8 | batch expansion across all categories | ~+687 | ✅ |
| 11.9 | final push to v0.1 gate (1139→5000+) | ~+3200 | ✅ |
| 11.10 | §25 deep review + v0.1 release prep | 0 (review only) | ✅ |

## Test runner

```bash
# Run all conformance tests (auto-detects parse vs compile mode per test path)
python3 tests/conformance/run_all.py --mode auto

# Run a single category
python3 tests/conformance/run_all.py --category 01-typecheck

# Run rust unit tests (includes stage11_*.rs)
cargo test --test all_tests
```

## Related documentation

- **Development docs**: `docs/develop/v0/stage-11/plan-11.{1..10}.md`
- **Gate reviews**: `docs/develop/v0/stage-11/gate-review-11.{1..10}.md`
- **Cross-stage audit**: `docs/develop/v0/stage-0-3-cross-stage-audit.md`
- **API naming standard**: `docs/develop/v0/api-naming-standard.md`
- **Project README**: `README.md` (top-level)
