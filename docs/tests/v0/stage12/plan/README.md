# Stage 12 — Test Documentation

> **阶段范围**: Stage 12.1 (v0.1 release + v0.3 bootstrap prep)
> **测试目录**: `tests/v0/stage12/plan/` + `tests/conformance/01-07-*`
> **状态**: 🔄 In Progress (12.1 — v0.1 release prep)

## 测试目录结构

```
tests/v0/stage12/plan/
├── README.md                    ← 本文件
└── stage12_1_tests.rs           (Stage 12.1 — v0.1 release + v0.3 bootstrap prep verification)

tests/conformance/               ← v0.1 conformance suite (target 5000, achieved 5026)
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

## Sub-stage overview

| Sub-stage | Focus | Result |
|-----------|-------|--------|
| 12.1 | v0.1 release doc + v0.3 bootstrap prep + cross-stage directory audit | ✅ |

## v0.1 release verification (Stage 12.1)

Stage 12.1 verifies the v0.1 release artifacts and conformance gate:

- `docs/develop/v0/stage-12/v0.1-release.md` exists and mentions "GATE REACHED" + "5026"
- `docs/develop/v0/stage-12/v0.3-bootstrap-prep.md` exists (planning document for Stage 1 bootstrap)
- All 12 stage test directories exist (`tests/v0/stage{0..12}`)
- All 13 stage develop directories exist (`docs/develop/v0/stage-{0..12}`)
- All 13 stage test-doc directories exist (`docs/tests/v0/stage{0..12}/plan`)
- README.md mentions v0.1 / v0.20 and references 5026 conformance tests
- Conformance gate still holds: ≥ 5000 tests (current: 5026)

## Related documentation

- **v0.1 release**: `docs/develop/v0/stage-12/v0.1-release.md`
- **v0.3 bootstrap prep**: `docs/develop/v0/stage-12/v0.3-bootstrap-prep.md`
- **Development plan**: `docs/develop/v0/stage-12/plan-12.1.md`
- **Gate review**: `docs/develop/v0/stage-12/gate-review-12.1.md`
- **Stage 11 closure**: `docs/develop/v0/stage-11/plan-11.10.md` (v0.1 gate reached)
- **Project README**: `README.md` (top-level)

## Test runner

```bash
# Run all conformance tests (auto-detects parse vs compile mode per test path)
python3 tests/conformance/run_all.py --mode auto

# Run rust unit tests (includes stage12_*.rs)
cargo test --test all_tests
```
