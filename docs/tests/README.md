# Test Documentation Index

> **Process**: v3.21 §17.2 — 每个测试代码文件必须有对应的测试文档（双向印证）
> **Stage 5.5 update**: tests/ refactored — flat files removed, unified
> `all_tests.rs` entry point + `autotests = false` in Cargo.toml.
> **Stage 8.7 update**: docs/tests/v0/stage{6,7,8}/ directories created and
> populated per §17.2 — full §17.1/§17.2/§17.3 compliance achieved.

## Directory Structure

```
docs/tests/
├── README.md                         (本文件 — 索引)
├── matrix.md                         (全局测试矩阵 — 覆盖率追踪)
└── v0/
    ├── stage0/plan/                  (Stage 0: Lexer + Parser + AST)
    │   ├── lexer.md                  ↔ tests/v0/stage0/plan/lexer_tests.rs
    │   ├── parser.md                 ↔ tests/v0/stage0/plan/parser_tests.rs
    │   └── ast_structure.md          ↔ tests/v0/stage0/plan/ast_structure_tests.rs
    ├── stage1/plan/                  (Stage 1: HIR + Name Resolution)
    │   ├── hir_structure.md          ↔ tests/v0/stage1/plan/hir_structure_tests.rs
    │   ├── hir_lowering.md           ↔ tests/v0/stage1/plan/hir_lowering_tests.rs
    │   ├── hir_resolution.md         ↔ tests/v0/stage1/plan/hir_resolution_tests.rs
    │   └── hir_scope_resolution.md   ↔ tests/v0/stage1/plan/hir_scope_resolution_tests.rs
    ├── stage2/plan/                  (Stage 2: MIR + Typeck + Borrowck)
    │   ├── mir_lowering.md           ↔ tests/v0/stage2/plan/mir_lowering_tests.rs
    │   ├── typeck.md                 ↔ tests/v0/stage2/plan/typeck_tests.rs
    │   ├── integration.md            ↔ tests/v0/stage2/plan/integration_tests.rs
    │   └── negative_cases.md         ↔ tests/v0/stage2/plan/negative_cases_tests.rs
    ├── stage3/plan/                  (Stage 3: LLVM Codegen)
    │   ├── codegen.md                ↔ tests/v0/stage3/plan/codegen_tests.rs
    │   ├── deep_inspection.md        ↔ tests/v0/stage3/plan/deep_inspection_tests.rs
    │   ├── codegen_basic.md          (历史文档 — 保留)
    │   ├── codegen_enum.md           (历史文档 — 保留)
    │   └── codegen_struct.md         (历史文档 — 保留)
    ├── stage4/                       (Stage 4: Modules + Closures + Macros)
    │   ├── plan/
    │   │   ├── stage4_features.md    (Stage 4.1-4.4 测试计划)
    │   │   └── closure_capture.md    ↔ tests/v0/stage4/plan/closure_capture_tests.rs
    │   └── gate/
    │       ├── gate-review-round1.md (Stage 4.1-4.5 审查)
    │       └── gate-review-round2.md (Stage 4.7 审查)
    ├── stage5/                       (Stage 5: TraitResolver + Vtable + stdlib)
    │   ├── plan/                     (92 test files, 642 tests)
    │   └── gate/
    ├── stage6/plan/                  (Stage 6: pure refactor, no new tests)
    │   └── README.md                 ← placeholder; 1881 tests unchanged
    ├── stage7/plan/                  (Stage 7: region inference + user-defined trait dyn)
    │   ├── README.md
    │   ├── region_inference.md       ↔ tests/v0/stage7/plan/region_inference_tests.rs (8 tests)
    │   ├── user_defined_trait_dyn.md ↔ tests/v0/stage7/plan/user_defined_trait_dyn_tests.rs (8 tests)
    │   ├── design_writeback_verification.md ↔ tests/v0/stage7/plan/design_writeback_verification_tests.rs (6 tests)
    │   ├── deep_review.md            ↔ tests/v0/stage7/plan/deep_review_tests.rs (5 tests)
    │   └── systematic_review_v014.md ↔ tests/v0/stage7/plan/systematic_review_v014_tests.rs (7 tests)
    └── stage8/plan/                  (Stage 8: v0.2 features + docs standardization)
        ├── README.md
        ├── lifetime_elision.md       ↔ tests/v0/stage8/plan/lifetime_elision_tests.rs (7 tests)
        ├── object_safety.md          ↔ tests/v0/stage8/plan/object_safety_tests.rs (5 tests)
        ├── extern_c_abi.md           ↔ tests/v0/stage8/plan/extern_c_abi_tests.rs (5 tests)
        ├── drop_elaboration.md       ↔ tests/v0/stage8/plan/drop_elaboration_tests.rs (7 tests)
        ├── async_await.md            ↔ tests/v0/stage8/plan/async_await_tests.rs (5 tests)
        └── deep_review.md            ↔ tests/v0/stage8/plan/deep_review_tests.rs (9 tests)
    └── stage9/plan/                  (Stage 9: v0.1 conformance suite expansion)
        ├── README.md
        ├── systematic_review_v0156.md ↔ tests/v0/stage9/plan/systematic_review_v0156_tests.rs (11 tests)
        ├── operators.md              ↔ tests/v0/stage9/plan/operators_tests.rs (11 tests)
        ├── control_flow.md           ↔ tests/v0/stage9/plan/control_flow_tests.rs (14 tests)
        ├── patterns.md               ↔ tests/v0/stage9/plan/patterns_tests.rs (16 tests)
        ├── types.md                  ↔ tests/v0/stage9/plan/types_tests.rs (14 tests)
        ├── attributes.md             ↔ tests/v0/stage9/plan/attributes_tests.rs (10 tests)
        ├── generics.md               ↔ tests/v0/stage9/plan/generics_tests.rs (10 tests)
        ├── closures.md               ↔ tests/v0/stage9/plan/closures_tests.rs (11 tests)
        ├── modules.md                ↔ tests/v0/stage9/plan/modules_tests.rs (10 tests)
        └── error_recovery.md          ↔ tests/v0/stage9/plan/error_recovery_tests.rs (8 tests)
```

## Test Code Directory Structure (tests/)

**Stage 5.5 refactor**: legacy flat `tests/*.rs` files (14 files, 11489 lines)
removed — they were duplicates of the organized `tests/v0/stage{N}/plan/`
files. The unified entry point `tests/all_tests.rs` includes all test
files via `#[path] mod` declarations. `Cargo.toml` sets `autotests = false`
so only one `[[test]]` target (`all_tests`) is built.

**Stage 8.7 update**: `tests/v0/stage6/plan/` directory created (placeholder
README only — Stage 6 was pure architectural refactoring, 1881 tests unchanged).

```
tests/
├── all_tests.rs                      ← unified entry point (#[path] mod declarations)
├── common/mod.rs                     ← shared test helpers
├── conformance/                      ← .lin conformance suite + run_all.py
└── v0/
    ├── stage0/plan/   (3 files)
    ├── stage1/plan/   (4 files)
    ├── stage2/plan/   (4 files)
    ├── stage3/plan/   (2 files)
    ├── stage4/plan/   (5 files)
    ├── stage5/plan/   (92 files)
    ├── stage6/plan/   (1 README — placeholder, no new tests)
    ├── stage7/plan/   (5 files: region_inference, user_defined_trait_dyn,
    │                          design_writeback_verification, deep_review,
    │                          systematic_review_v014)
    └── stage8/plan/   (6 files: lifetime_elision, object_safety, extern_c_abi,
                                 drop_elaboration, async_await, deep_review)
    └── stage9/plan/   (10 files: systematic_review_v0156_tests, operators_tests, control_flow_tests, patterns_tests, types_tests, attributes_tests, generics_tests, closures_tests, modules_tests, error_recovery_tests)
```

### Why the refactor?

Before Stage 5.5, `Cargo.toml` had 19 `[[test]]` entries (one per file),
and `tests/` had 14 duplicate flat `.rs` files alongside the organized
`tests/v0/` tree. The refactor:

1. **Removes duplicates** — flat files were 100% duplicates of organized files
2. **Shrinks Cargo.toml** — 19 `[[test]]` entries → 1 (71% line reduction)
3. **Single test binary** — faster incremental compilation (one link step)
4. **No Cargo.toml edit for new tests** — just add a `#[path]` line in `all_tests.rs`

### Running tests

```bash
# Run all tests (1017 expected — 1013 baseline + 3 vtable + 1 audit)
cargo test

# Run a single module (e.g. only lexer tests)
cargo test --test all_tests -- lexer_tests

# Run a single test function
cargo test --test all_tests -- lexer_tests::test_int_decimal
```

## Migration History

- **Stage 4.8 (v0.9.5)**: Full restructure — all 13 flat `tests/*.rs` files
  migrated to standardized `tests/v0/stage{N}/plan/` per v3.17 §17.1.
  - 27 markdown files updated with new test paths
  - 14 `[[test]]` targets in Cargo.toml
  - 993 tests pass (100% coverage of original)

- **Stage 5.5 (v0.11.4)**: Test infrastructure refactor
  - Removed 14 legacy flat `tests/*.rs` files (11489 lines, duplicates)
  - Created `tests/all_tests.rs` unified entry point (23 `#[path] mod`)
  - `Cargo.toml`: `autotests = false` + single `[[test]]` entry
  - Cargo.toml test section: 19 entries → 1 entry (71% line reduction)
  - Test count unchanged: 1017 (no test logic touched)

## Total Test Count

| Stage | Tests | Files |
|-------|-------|-------|
| Stage 0 | 344 | 3 |
| Stage 1 | 117 | 4 |
| Stage 2 | 170 | 4 |
| Stage 3 | 309 | 2 |
| Stage 4 | 67 (incl. 5 bench) | 5 |
| Stage 5 | 642 | 92 |
| Stage 6 | — (refactor, behavior-equivalent) | — |
| Stage 7 | 154 (+28 unit) | 5 |
| Stage 8 | 38 (+9 unit) | 6 |
| Stage 9 | +114 rust + +539 conformance | 10 rust + 539 .lin |
| **Total** | **2215** rust + **547** conformance (146 unit + 2069 integration, 2 ignored) | **127** rust + **547** .lin |

---

**Last updated**: 2026-07-26 (Stage 9.10 — Error recovery conformance expansion)
**Process**: v3.21
