# Stage 13 — Test Documentation

> **阶段范围**: Stage 13.1 - 13.13+ (v0.3 self-hosting preparation + LLVM execution pipeline)
> **测试目录**: `tests/v0/stage13/plan/` + `tests/conformance/01-07-*`
> **状态**: 🔄 In Progress (13.1 ✅, 13.2 ✅, 13.3a ✅, 13.4a ✅, 13.5-13.13 ✅; 13.14+ pending)

## 测试目录结构

```
tests/v0/stage13/plan/
├── README.md                    ← 本文件
├── stage13_1_tests.rs           (Stage 13.1 — audit/plan docs verification + cross-stage audit ratification)
├── stage13_2_tests.rs           (Stage 13.2 — if-let / while-let, 11 conformance FAIL→PASS)
├── stage13_3_tests.rs           (Stage 13.3 — closure call lowering preparation)
├── stage13_3a_tests.rs          (Stage 13.3a — TD-030 P0 CLOSED, closures callable inline)
├── stage13_4_tests.rs           (Stage 13.4 — built-in macros preparation + TD-032 reframe)
├── stage13_4a_tests.rs          (Stage 13.4a — TD-032 P0 CLOSED, all 26 built-in macros)
├── stage13_5_muv2_tests.rs      (Stage 13.5 MUV-2 — LLVMSysEmitter + LLVM version switching)
├── stage13_5_muv3_tests.rs      (Stage 13.5 MUV-3 — End-to-end LLVM module → object file)
├── stage13_8_tests.rs           (Stage 13.8 — --run flag + --emit-bin with auto C wrapper)
├── stage13_9_tests.rs           (Stage 13.9 — Comprehensive --run verification across constructs)
└── stage13_13_tests.rs          (Stage 13.13 — Inline println! emission via StatementKind::Println)

tests/conformance/               ← v0.1 conformance suite (Stage 13 不增加数量, 重点是 FAIL → PASS)
├── 00-parse/                    (612 — Stage 13.2 if-let/while-let tests flipped to PASS)
├── 01-typecheck/                (1061 — Stage 13.3a closure compile_error→compile_ok)
├── 02-borrowck/                 (830 — Stage 13.2/13.3a closure capture compile_error→compile_ok)
├── 03-codegen/                  (601, no change in Stage 13)
├── 04-e2e/                      (543 — Stage 13.3a closure e2e compile_error→compile_ok)
├── 05-soundness/                (500, no change in Stage 13)
├── 06-stdlib/                   (502, no change in Stage 13)
└── 07-integration/              (501, no change in Stage 13)
```

## Sub-stage overview

| Sub-stage | Focus | Tests added | Conformance FAIL→PASS |
|-----------|-------|-------------|----------------------|
| 13.1 | Architecture baseline (TD-028 + TD-029 + 6 missing READMEs backfilled) | +10 rust | 0 |
| 13.2 | if-let / while-let (TD-031 P0 CLOSED) | +11 rust | +11 (5015→5026) |
| 13.3 | Closure call lowering preparation (TD-030) | +9 rust | 0 |
| 13.3a | TD-030 P0 CLOSED — closures callable (inline approach) | +9 rust | +30 compile_error→compile_ok |
| 13.4 | Built-in macros preparation + TD-032 reframe | +7 rust | 0 |
| 13.4a | TD-032 P0 CLOSED — all 26 built-in macros | +8 rust | 0 |
| 13.5 MUV-1 | LLVM library integration (llvm-sys linked) | +6 rust | 0 |
| 13.5 MUV-2 | LLVMSysEmitter (36/36 Emitter methods) | +9 rust | 0 |
| 13.5 MUV-3 | LLVM module → object file e2e | +N rust | 0 |
| 13.6 | `--emit-obj` flag | +N rust | 0 |
| 13.7-13.10 | `--emit-bin` + auto C wrapper + `--run` + runtime stubs | +N rust | 0 |
| 13.11-13.12 | println! capture + side-table emission (with known limitation) | +N rust | 0 |
| 13.13 | Inline println! emission via `StatementKind::Println` (fixes 13.12 ordering bug) | +10 rust | 0 |
| 13.5+ | TD-033 P1 sub-items (for/move/HRTB/assoc-norm/two-phase/RFC 2229) | TBD | TBD |
| 13.6 (release) | v0.1 release announcement | 0 | 0 |

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
