# Test Plan: Stage 5.41 — Stdlib Vtable Emission Plan (Aggregate)

> **Stage**: 5.41
> **Version**: v0.11.36 → v0.11.37
> **Test file**: `tests/v0/stage5/plan/stdlib_vtable_emission_tests.rs`
> **Test count**: 17 new tests (1206 → 1223 total)
> **Process ref**: stage-committee-process.md v3.20 §17.3 时期 2

## 1. 测试目标

验证 `StdlibVtableEmission` struct + `stdlib_vtable_emission()` +
`stdlib_vtable_emissions_for_traits()` 的正确性。

## 2. 覆盖场景

### 2.1 单 emission 构造

- Clone + S + [clone, clone_from] → 2 slots, complete, not marker
- Clone + S + [clone] → 2 slots, not complete (clone_from missing)
- Drop + S + [drop] → 1 slot, complete
- Copy + S + [] → 0 slots, marker, vacuously complete
- BogusTrait/From/"" → None
- Add + Vec + [add] → 1 slot arith

### 2.2 字段正确性

- global_name = `.vtable.<trait>.<type>`
- byte_size_32 / byte_size_64 for various slot counts (2/1/0)
- is_complete true/false
- is_marker true for 6 markers, false for Clone/Add/etc

### 2.3 批量查询

- Clone + Drop → 2 emissions, both complete
- 未知 trait 静默跳过（BogusTrait/From 不出现在结果中）
- 空 trait 列表 → 空 Vec
- markers 包含在结果中（is_marker=true）

### 2.4 结构体语义

- PartialEq/Eq 派生
- 9 个字段全部可访问

## 3. 测试统计

- 新增: 17 tests
- 基线: 1206 tests
- 总计: 1223 tests
- 2 ignored (pre-existing, 未影响)

## 4. 依赖

- 上游:
  - Stage 5.36 (`StdlibTraitMethod`)
  - Stage 5.37 (`stdlib_vtable_slot_count`)
  - Stage 5.40 (`stdlib_vtable_global_name` + `stdlib_vtable_method_symbols`)
- 下游:
  - Stage 5.42+ (codegen vtable emission refactor) — codegen 调用
    `stdlib_vtable_emission()` 一次，直接消费字段
  - Stage 5.43+ (dyn Trait MIR lowering) — 批量查询

## 5. CI/CD 验证

```
cargo clean: clean (801.5 MiB removed)
cargo test: 1223 passed, 0 failed, 2 ignored ✅
cargo fmt --check: clean ✅
cargo clippy --all-targets: 0 warnings ✅
```

---

**创建日期**: 2026-07-23
