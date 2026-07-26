# Stage 3 — Test Documentation

> **阶段范围**: Stage 3.1 - 3.69 (LLVM IR codegen MVP → typed codegen + runtime checks)
> **测试目录**: `tests/v0/stage3/plan/` + `tests/conformance/03-codegen/`
> **状态**: ✅ Complete

## 测试目录结构

```
tests/v0/stage3/plan/
├── README.md                    ← 本文件
└── codegen_tests.rs             (Stage 3 — LLVM IR codegen verification)

tests/conformance/03-codegen/    ← Stage 11.3 expanded to 601 tests (100.2% ✅)
├── 00-basic/                    (basic codegen patterns)
├── 01-arithmetic/               (arithmetic operations)
├── 02-control-flow/             (control flow codegen)
├── 03-functions/                (function calls + ABI)
├── 04-adt/                      (struct/enum codegen)
├── 05-closures/                 (closure codegen)
└── 06-traits/                   (trait dispatch codegen)
```

## 测试统计

| Type | Count |
|------|-------|
| Rust integration tests | 309 |
| Conformance tests (03-codegen) | 601 (100.2% of 600 target) ✅ |

## 测试覆盖 (Rust integration)

| Module | Tests | Focus |
|--------|-------|-------|
| codegen_tests.rs | 309 | LLVM IR emission, types, function calls, ABI, structs, enums, error recovery |

## Conformance per subcategory (03-codegen)

| Subcategory | Count | Status |
|-------------|-------|--------|
| 00-basic | 100 | ✅ |
| 01-arithmetic | 100 | ✅ |
| 02-control-flow | 100 | ✅ |
| 03-functions | 100 | ✅ |
| 04-adt | 100 | ✅ |
| 05-closures | 50 | ✅ |
| 06-traits | 51 | ✅ |
| **Total** | **601** | **100.2% ✅** |

## 关联文档

- `docs/develop/v0/stage-3/dev-log.md` — Stage 3 开发日志
- `docs/develop/v0/stage-3/gate-review-round1.md` to `gate-review-round10.md` — 10 轮门审查
- `docs/develop/v0/stage-3/deep-review-r37.md` — Stage 3.37 深度审查
- `docs/develop/v0/stage-0-3-cross-stage-audit.md` — 跨阶段审查
- `docs/lang-design/07-codegen.md` — Codegen 设计
- `docs/lang-design/08-codegen.md` — Codegen 实现细节
- `docs/tests/v0/stage3/plan/{codegen,codegen_basic,codegen_enum}.md` — 各模块测试设计文档

## 测试 runner

```bash
cargo test --test all_tests -- codegen_tests
python3 tests/conformance/run_all.py --category 03-codegen
```
