# Test Documentation Index

> **Process**: v3.17 §17.2 — 每个测试代码文件必须有对应的测试文档（双向印证）
> **Stage 5.5 update**: tests/ refactored — flat files removed, unified
> `all_tests.rs` entry point + `autotests = false` in Cargo.toml.

## Directory Structure

```
docs/tests/
├── README.md                         (本文件 — 索引)
├── matrix.md                         (全局测试矩阵 — 覆盖率追踪)
└── v0/
    ├── stage0/plan/                  (Stage 0: Lexer + Parser + AST)
    │   ├── lexer.md                  ↔ tests/v0/stage0/plan/lexer_tests.rs (109 tests)
    │   ├── parser.md                 ↔ tests/v0/stage0/plan/parser_tests.rs (85 tests)
    │   └── ast_structure.md          ↔ tests/v0/stage0/plan/ast_structure_tests.rs (150 tests)
    ├── stage1/plan/                  (Stage 1: HIR + Name Resolution)
    │   ├── hir_structure.md          ↔ tests/v0/stage1/plan/hir_structure_tests.rs (20 tests)
    │   ├── hir_lowering.md           ↔ tests/v0/stage1/plan/hir_lowering_tests.rs (36 tests)
    │   ├── hir_resolution.md         ↔ tests/v0/stage1/plan/hir_resolution_tests.rs (26 tests)
    │   └── hir_scope_resolution.md   ↔ tests/v0/stage1/plan/hir_scope_resolution_tests.rs (17 tests)
    ├── stage2/plan/                  (Stage 2: MIR + Typeck + Borrowck)
    │   ├── mir_lowering.md           ↔ tests/v0/stage2/plan/mir_lowering_tests.rs (22 tests)
    │   ├── typeck.md                 ↔ tests/v0/stage2/plan/typeck_tests.rs (26 tests)
    │   ├── integration.md            ↔ tests/v0/stage2/plan/integration_tests.rs (58 tests)
    │   └── negative_cases.md         ↔ tests/v0/stage2/plan/negative_cases_tests.rs (35 tests)
    ├── stage3/plan/                  (Stage 3: LLVM Codegen)
    │   ├── codegen.md                ↔ tests/v0/stage3/plan/codegen_tests.rs (294 tests)
    │   ├── deep_inspection.md        ↔ tests/v0/stage3/plan/deep_inspection_tests.rs (15 tests)
    │   ├── codegen_basic.md          (历史文档 — 保留)
    │   ├── codegen_enum.md           (历史文档 — 保留)
    │   └── codegen_struct.md         (历史文档 — 保留)
    ├── stage4/                       (Stage 4: Modules + Closures + Macros)
    │   ├── plan/
    │   │   ├── stage4_features.md    (Stage 4.1-4.4 测试计划)
    │   │   └── closure_capture.md    ↔ tests/v0/stage4/plan/closure_capture_tests.rs (4 tests)
    │   └── gate/
    │       ├── gate-review-round1.md (Stage 4.1-4.5 审查)
    │       └── gate-review-round2.md (Stage 4.7 审查)
    └── stage5/                       (Stage 5: TraitResolver + Vtable)
        ├── plan/
        │   ├── trait_resolver.md     ↔ tests/v0/stage5/plan/trait_resolver_tests.rs
        │   ├── ty_is_copy.md         ↔ tests/v0/stage5/plan/ty_is_copy_tests.rs
        │   ├── def_id_name_map.md    ↔ tests/v0/stage5/plan/def_id_name_map_tests.rs
        │   └── vtable.md             ↔ tests/v0/stage5/plan/vtable_tests.rs (4 tests)
        └── gate/
            ├── gate-review-round1.md … gate-review-round5.md
```

## Test Code Directory Structure (tests/)

**Stage 5.5 refactor**: legacy flat `tests/*.rs` files (14 files, 11489 lines)
removed — they were duplicates of the organized `tests/v0/stage{N}/plan/`
files. The unified entry point `tests/all_tests.rs` includes all 23 test
files via `#[path] mod` declarations. `Cargo.toml` sets `autotests = false`
so only one `[[test]]` target (`all_tests`) is built.

```
tests/
├── all_tests.rs                      ← unified entry point (23 #[path] mod)
├── common/mod.rs                     ← shared test helpers
├── conformance/                      ← .lin conformance suite + run_all.py
└── v0/
    ├── stage0/plan/  (3 files: lexer, parser, ast_structure)
    ├── stage1/plan/  (4 files: hir_structure, hir_lowering, hir_resolution, hir_scope_resolution)
    ├── stage2/plan/  (4 files: mir_lowering, typeck, integration, negative_cases)
    ├── stage3/plan/  (2 files: codegen, deep_inspection)
    ├── stage4/plan/  (5 files: closure_capture, closure_call, closure_full_call, macro_system, visibility)
    └── stage5/plan/  (5 files: trait_resolver, trait_integration, ty_is_copy, def_id_name_map, vtable)
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
| Stage 1 | 99 | 4 |
| Stage 2 | 141 | 4 |
| Stage 3 | 309 | 2 |
| Stage 4 | 4 | 1 |
| Stage 5 | 14 | 5 |
| **Total** | **1017** | **23** |

---

**Last updated**: 2026-07-22 (Stage 5.5 — tests/ refactor + Cargo.toml cleanup)
**Process**: v3.18
