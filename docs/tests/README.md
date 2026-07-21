# Test Documentation Index

> **Process**: v3.17 §17.2 — 每个测试代码文件必须有对应的测试文档（双向印证）

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
    └── stage4/                       (Stage 4: Modules + Closures + Macros)
        ├── plan/
        │   ├── stage4_features.md    (Stage 4.1-4.4 测试计划)
        │   └── closure_capture.md    ↔ tests/v0/stage4/plan/closure_capture_tests.rs (4 tests)
        └── gate/
            ├── gate-review-round1.md (Stage 4.1-4.5 审查)
            └── gate-review-round2.md (Stage 4.7 审查)
```

## Test Code Directory Structure (tests/)

```
tests/
├── v0/
│   ├── stage0/plan/
│   │   ├── lexer_tests.rs            (109 tests)
│   │   ├── parser_tests.rs           (85 tests)
│   │   └── ast_structure_tests.rs    (150 tests)
│   ├── stage1/plan/
│   │   ├── hir_structure_tests.rs    (20 tests)
│   │   ├── hir_lowering_tests.rs     (36 tests)
│   │   ├── hir_resolution_tests.rs   (26 tests)
│   │   └── hir_scope_resolution_tests.rs (17 tests)
│   ├── stage2/plan/
│   │   ├── mir_lowering_tests.rs     (22 tests)
│   │   ├── typeck_tests.rs           (26 tests)
│   │   ├── integration_tests.rs      (58 tests)
│   │   └── negative_cases_tests.rs   (35 tests)
│   ├── stage3/plan/
│   │   ├── codegen_tests.rs          (294 tests)
│   │   └── deep_inspection_tests.rs  (15 tests)
│   └── stage4/plan/
│       └── closure_capture_tests.rs  (4 tests)
└── common/                           (shared test helpers — tests/common/mod.rs)
```

## Migration History

- **Stage 4.8 (v0.9.5)**: Full restructure — all 13 flat `tests/*.rs` files
  migrated to standardized `tests/v0/stage{N}/plan/` per v3.17 §17.1.
  - 0 flat .rs files remain in tests/ root
  - 0 empty directories
  - `tests/common/mod.rs` shared helper module created
  - 27 markdown files updated with new test paths
  - 14 `[[test]]` targets in Cargo.toml
  - 993 tests pass (100% coverage of original)

## Total Test Count

| Stage | Tests | Files |
|-------|-------|-------|
| Stage 0 | 344 | 3 |
| Stage 1 | 99 | 4 |
| Stage 2 | 141 | 4 |
| Stage 3 | 309 | 2 |
| Stage 4 | 4 | 1 |
| **Total** | **993** | **14** |

---

**Last updated**: 2026-07-22 (Stage 4.8)
**Process**: v3.17
