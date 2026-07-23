# Test Plan: Stage 5.39 — Stdlib Vtable Construction Planner

> **Stage**: 5.39
> **Version**: v0.11.34 → v0.11.35
> **Test file**: `tests/v0/stage5/plan/stdlib_vtable_plan_tests.rs`
> **Test count**: 18 new tests (1172 → 1190 total)
> **Process ref**: stage-committee-process.md v3.20 §17.3 时期 2

## 1. 测试目标

验证 `StdlibVtablePlanEntry` + `StdlibVtablePlan` + `stdlib_vtable_plan()` +
`stdlib_vtable_plan_entry_count()` + `stdlib_vtable_plan_is_complete()` +
`stdlib_vtable_plan_missing_methods()` 的正确性。

## 2. 覆盖场景

### 2.1 plan 构造

- Clone + [clone, clone_from] → 完整 plan (2 entries, all provided)
- Clone + [clone] → 部分 plan (clone_from missing)
- Drop + [drop] → 1 entry complete
- PartialEq + [eq] → ne missing
- Add + [add] → complete
- Copy + [] → 空 plan (marker), vacuously complete
- BogusTrait/From/"" → None
- Clone + [clone, bogus, another_extra] → bogus 不影响 plan (宽容设计)

### 2.2 entry_count

- Clone=2, Drop=1, PartialEq=2, Add=1, Copy=0
- BogusTrait=None

### 2.3 is_complete

- 完整 plan → true (method 形式 + free fn 形式)
- 部分 plan → false (Clone + [clone], PartialEq + [])
- 空 plan (marker) → true (vacuously)

### 2.4 missing_methods

- 完整 plan → 空 Vec
- Clone + [clone] → ["clone_from"]
- PartialEq + [] → ["eq", "ne"] (slot 顺序)

### 2.5 determinism + struct semantics

- 重复调用同一 (trait, provided) → 相同 plan (PartialEq)
- StdlibVtablePlanEntry 字段访问
- entries 按 slot_index 升序排列

## 3. 测试统计

- 新增: 18 tests
- 基线: 1172 tests
- 总计: 1190 tests
- 2 ignored (pre-existing, 未影响)

## 4. 依赖

- 上游:
  - Stage 5.36 (`StdlibTraitMethod` + `stdlib_trait_methods()`)
  - Stage 5.37 (`stdlib_vtable_slot_count()`)
- 下游:
  - Stage 5.40+ (dyn Trait codegen) — 将使用 `stdlib_vtable_plan()` 一次
    生成有序 entries，遍历填入 LLVM symbol
  - Stage 5.41+ (typeck impl completeness) — 将使用
    `stdlib_vtable_plan_missing_methods()` 报告未实现方法

## 5. CI/CD 验证

```
cargo clean: clean (916.7 MiB removed)
cargo test: 1190 passed, 0 failed, 2 ignored ✅
cargo fmt --check: clean ✅
cargo clippy --all-targets: 0 warnings ✅
```

---

**创建日期**: 2026-07-23
