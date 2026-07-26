# Stage 5 — Test Documentation

> **阶段范围**: Stage 5.1 - 5.66 (99 sub-stages: TraitResolver + vtable + dyn Trait + stdlib)
> **测试目录**: `tests/v0/stage5/plan/` + `tests/conformance/06-stdlib/`
> **状态**: ✅ Complete

## 测试目录结构

```
tests/v0/stage5/plan/             ← 92 .rs files, 977 #[test] items
├── README.md                     ← 本文件
├── builtin_clone_drop_tests.rs
├── builtin_copy_activation_tests.rs
├── builtin_traits_tests.rs
├── coherence_tests.rs
├── dyn_trait_fat_ptr_batch_tests.rs
├── dyn_trait_method_call_tests.rs
├── ... (88 more files)
└── vtable_query_tests.rs

tests/conformance/06-stdlib/      ← Stage 11.6 expanded to 502 tests (100.4% ✅)
├── 00-core/                      (50 tests)
├── 01-alloc/                     (50 tests)
├── 02-collections/               (50 tests)
├── 03-closures/                  (50 tests)
├── 04-traits/                    (50 tests)
├── 05-string/                    (50 tests)
├── 06-vtable/                    (50 tests)
├── 07-io/                        (50 tests)
├── 08-math/                      (50 tests)
└── 09-sync/                      (50 tests)
```

## 测试统计

| Type | Count |
|------|-------|
| Rust integration tests | 977 (across 92 .rs files) |
| Conformance tests (06-stdlib) | 502 (100.4% of 500 target) ✅ |

## 测试覆盖 (主要模块)

| Module | Tests | Focus |
|--------|-------|-------|
| builtin_traits_tests.rs | 30 | Copy/Clone/Drop/Send/Sync auto traits |
| builtin_copy_activation_tests.rs | 25 | Copy activation rules |
| builtin_clone_drop_tests.rs | 30 | Clone/Drop codegen |
| vtable_query_tests.rs | 25 | TraitResolver vtable queries |
| vtable_layout_tests.rs | 30 | Vtable byte layout, slot offsets |
| dyn_trait_fat_ptr_batch_tests.rs | 25 | dyn Trait fat pointer emission |
| dyn_trait_method_call_tests.rs | 30 | dyn Trait method dispatch |
| coherence_tests.rs | 20 | Trait coherence checking |
| impl_completeness_tests.rs | 20 | Trait impl completeness check |
| object_safety_tests.rs | 25 | Object safety rules (Stage 8.2) |
| stdlib_layout_tests.rs | 30 | Stdlib type layout |
| stdlib_trait_method_tests.rs | 35 | Stdlib trait method signatures |
| stdlib_vtable_layout_tests.rs | 25 | Stdlib vtable slot layout |
| (other 79 files) | 607 | Various trait/stdlib/dyn aspects |

## Conformance per subcategory (06-stdlib)

| Subcategory | Count | Status |
|-------------|-------|--------|
| 00-core | 50 | ✅ |
| 01-alloc | 50 | ✅ |
| 02-collections | 50 | ✅ |
| 03-closures | 50 | ✅ |
| 04-traits | 50 | ✅ |
| 05-string | 50 | ✅ |
| 06-vtable | 50 | ✅ |
| 07-io | 50 | ✅ |
| 08-math | 50 | ✅ |
| 09-sync | 50 | ✅ |
| **Total** | **502** | **100.4% ✅** |

## 关联文档

- `docs/develop/v0/stage-5/` — Stage 5 开发文档 (200 files: dev-logs, gate-reviews, deep-reviews)
- `docs/develop/v0/stage-5/deep-review-r100.md` / `r110.md` / `r120.md` — 多轮深度审查
- `docs/lang-design/09-stdlib.md` — Stdlib 设计
- `docs/lang-design/03-type-system.md` §2 — Trait 系统设计
- `docs/tests/v0/stage5/plan/*.md` — 70 个测试设计文档 (builtin_clone_drop.md, builtin_copy_activation.md, ...)

## 测试 runner

```bash
# 所有 Stage 5 测试 (mods starting with stage5_*)
cargo test --test all_tests -- stage5_

# Conformance stdlib category
python3 tests/conformance/run_all.py --category 06-stdlib
```
