# Stage 13 — Test Documentation

> **阶段范围**: Stage 13.1 - 13.6 (v0.3 self-hosting preparation)
> **测试目录**: `tests/v0/stage13/plan/` + `tests/conformance/01-07-*`
> **状态**: 🔄 In Progress (13.1 — architecture baseline + audit/plan verification)

## 测试目录结构

```
tests/v0/stage13/plan/
├── README.md                    ← 本文件
├── stage13_1_tests.rs           (Stage 13.1 — audit/plan docs verification + cross-stage audit ratification)
├── stage13_2_tests.rs           (Stage 13.2 — if-let / while-let, planned)
├── stage13_3_tests.rs           (Stage 13.3 — closure call lowering, planned)
└── stage13_4_tests.rs           (Stage 13.4 — macro_rules! + 26 built-in macros, planned)

tests/conformance/               ← v0.1 conformance suite (Stage 13 不增加数量, 重点是 FAIL → PASS)
├── 00-parse/                    (600 → 612 expected after Stage 13.2 if-let parse tests)
├── 01-typecheck/                (1020 → 1061 expected after Stage 13.3 closure call tests)
├── 02-borrowck/                 (800 → 830 expected after Stage 13.2/13.3)
├── 03-codegen/                  (601, no change in Stage 13)
├── 04-e2e/                      (502 → 543 expected after Stage 13.3)
├── 05-soundness/                (500, no change in Stage 13)
├── 06-stdlib/                   (502, no change in Stage 13)
└── 07-integration/              (501, no change in Stage 13)
```

## Sub-stage overview

| Sub-stage | Focus | Tests added | Conformance FAIL→PASS |
|-----------|-------|-------------|----------------------|
| 13.1 | Architecture baseline (TD-028 + TD-029 + 6 missing READMEs backfilled) | +5 rust | 0 |
| 13.2 | if-let / while-let (TD-031) | TBD | +12 |
| 13.3 | Closure call lowering (TD-030) | TBD | +41 |
| 13.4 | macro_rules! + 26 built-in macros (TD-032) | TBD | TBD |
| 13.5 | TD-033 P1 sub-items (for/move/HRTB/assoc-norm/two-phase/RFC 2229) | TBD | TBD |
| 13.6 | v0.1 release announcement | 0 | 0 |

## Stage 13.1 verification tests

`tests/v0/stage13/plan/stage13_1_tests.rs` verifies:

1. `cross-stage-audit-r216-architecture.md` exists in `docs/develop/v0/stage-12/`
2. `cross-stage-audit-r216-techdebt-tests-docs.md` exists in `docs/develop/v0/stage-12/`
3. `plan-13.1.md` exists in `docs/develop/v0/stage-13/`
4. Stage 13 independent directories exist (`tests/v0/stage13/plan`, `docs/develop/v0/stage-13`, `docs/tests/v0/stage13/plan`)
5. All 14 stage develop directories exist (stage-0 through stage-13)
6. All 14 stage test-doc directories exist (stage0 through stage13)
7. All 13 stage `plan/README.md` files exist (Stage 0-12 — Stage 13 just created)
8. `docs/lang-design/03-type-system.md` updated with §25.8 write-back (§13 added)
9. v0.1 conformance gate still holds (≥5000)
10. Tech debt inventory: 7 open items (P0=3, P1=1, P2=2, P3=1-on-hold)

## Related documentation

- **Stage 12 audit reports**:
  - `docs/develop/v0/stage-12/cross-stage-audit-r216-architecture.md` (ARCH-A, D1+D5)
  - `docs/develop/v0/stage-12/cross-stage-audit-r216-techdebt-tests-docs.md` (QA-A + REV-A + PM-A, D2+D3+D4+D6+D7)
- **Stage 13 plan**: `docs/develop/v0/stage-13/plan-13.1.md`
- **Stage 12 closure**: `docs/develop/v0/stage-12/v0.1-release.md` + `v0.3-bootstrap-prep.md`
- **Project README**: `README.md` (top-level)
- **Process**: `docs/stage-committee-process.md` v3.21

## Test runner

```bash
# Stage 13 verification tests
cargo test --test all_tests -- stage13_

# Full conformance suite
python3 tests/conformance/run_all.py --mode auto

# All rust tests
cargo test --test all_tests
```
