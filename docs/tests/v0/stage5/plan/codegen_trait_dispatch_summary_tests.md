# Test Plan: Stage 5.52 — Codegen Trait-Dispatch Emission Summary

> **Stage**: 5.52
> **Version**: v0.11.47 → v0.11.48
> **Test file**: `tests/v0/stage5/plan/codegen_trait_dispatch_summary_tests.rs`
> **Test count**: 14 new tests (1346 → 1360 total)
> **Process ref**: stage-committee-process.md v3.20 §17.3 时期 2

## 1. 测试目标

验证 `CodegenTraitDispatchEmissionSummary` struct +
`build_trait_dispatch_emission_summary()` free function 的正确性。

## 2. 覆盖场景

### 2.1 边界

- 空 TraitResolver → 全 0 + 空名列表
- 单个 vtable → 1 vtable + 1 dynptr + 1 trait + 1 type
- 多个 vtable → 多 vtable + 多 dynptr + 多 trait + 多 type

### 2.2 字段正确性

- vtable_count == vtables.len()
- dynptr_count == vtable_count
- total_global_count == vtable_count + dynptr_count
- trait_names 去重（同 trait 多 type → 1 trait）
- type_names 去重（同 type 多 trait → 1 type）
- total_method_slots == 所有 vtable.entries.len() 之和

### 2.3 边界情况

- interner 未找到 Spur → "Trait"/"Type" 默认名（用 fresh Rodeo 模拟）
- 纯函数——不修改输入 TraitResolver

### 2.4 真实场景 + 结构体语义

- 模拟真实 TraitResolver：S impls Clone+Drop+Display → 3 vtable + 3 dynptr + 4 slots
- PartialEq/Eq 派生
- 字段访问

## 3. 测试统计

- 新增: 14 tests
- 基线: 1346 tests
- 总计: 1360 tests
- 2 ignored (pre-existing, 未影响)

## 4. 依赖

- 上游:
  - Stage 5.42 (`stdlib_vtable_emission_summary` — stdlib 对应版本)
  - `TraitResolver` + `Vtable` + `VtableEntry` (Stage 5.5)
- 下游:
  - Stage 5.53 (codegen trait-dispatch emission refactor) — driver 调用
    summary 做诊断输出
  - Stage 5.54+ (dyn Trait MIR lowering) — 直接调用

## 5. CI/CD 验证

```
cargo clean: clean (838.6 MiB removed)
cargo test: 1360 passed, 0 failed, 2 ignored ✅
cargo fmt --check: clean ✅
cargo clippy --all-targets: 0 warnings ✅
```

---

**创建日期**: 2026-07-23
