# Stage 1 — Test Documentation

> **阶段范围**: Stage 1.1 - 1.3 (HIR data structures + lowering + name resolution)
> **测试目录**: `tests/v0/stage1/plan/`
> **状态**: ✅ Complete

## 测试目录结构

```
tests/v0/stage1/plan/
├── README.md                    ← 本文件
├── hir_structure_tests.rs       (Stage 1.1 — HIR data structures verification)
├── hir_lowering_tests.rs        (Stage 1.2 — AST → HIR lowering)
├── hir_resolution_tests.rs      (Stage 1.3 — name resolution)
└── hir_scope_resolution_tests.rs (Stage 1.3 — scope + use resolution)
```

## 测试统计

| Type | Count |
|------|-------|
| Rust integration tests | 99 |

## 测试覆盖

> Per-module counts verified by r217 second-pass audit (Stage 12.7 correction).
> Total = 99 ✅ (matches); per-module breakdown corrected from r216 first-pass estimates.

| Module | Tests | Focus |
|--------|-------|-------|
| hir_structure_tests.rs | 20 | HirId/DefId, HirItem variants, HirExpr, HirPat, HirTy, Res, InferTy |
| hir_lowering_tests.rs | 36 | AST → HIR transformation correctness |
| hir_resolution_tests.rs | 26 | Path resolution, use imports, visibility |
| hir_scope_resolution_tests.rs | 17 | Lexical scopes, shadowing, glob imports |
| **Total** | **99** | (verified r217) |

## 关联文档

- `docs/develop/v0/stage-1/dev-log.md` — Stage 1 开发日志
- `docs/develop/v0/stage-1/plan-1.1.md` — Stage 1.1 plan
- `docs/develop/v0/stage-1/plan-1.2.md` — Stage 1.2 plan
- `docs/develop/v0/stage-1/plan-1.3.md` — Stage 1.3 plan
- `docs/lang-design/03-type-system.md` — Type system design
- `docs/tests/v0/stage1/plan/{hir_lowering,hir_resolution,hir_scope_resolution}.md` — 各模块测试设计文档

## 测试 runner

```bash
cargo test --test all_tests -- hir_structure_tests hir_lowering_tests hir_resolution_tests hir_scope_resolution_tests
```
