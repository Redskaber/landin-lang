# Test Plan: Stage 5.42 — Stdlib Vtable Emission Summary

> **Stage**: 5.42
> **Version**: v0.11.37 → v0.11.38
> **Test file**: `tests/v0/stage5/plan/stdlib_vtable_emission_summary_tests.rs`
> **Test count**: 13 new tests (1223 → 1236 total)
> **Process ref**: stage-committee-process.md v3.20 §17.3 时期 2

## 1. 测试目标

验证 `StdlibVtableEmissionSummary` struct + `stdlib_vtable_emission_summary()`
的正确性。

## 2. 覆盖场景

### 2.1 边界

- 空输入 → 全 0 + 空 trait_names
- 单个完整 emission → counts/slots/byte_sizes 正确
- 单个 marker emission → marker_count=1, complete_count=1 (vacuous)

### 2.2 多 emission 混合

- 4 emissions (Clone complete + Drop complete + Copy marker + PartialEq incomplete)
  → 验证 total/marker/complete/incomplete/slots/byte_sizes 全部正确

### 2.3 字段累加

- total_slots 跨 emission 累加
- total_byte_size_32 / total_byte_size_64 跨 emission 累加

### 2.4 trait_names 去重

- 同 trait 多个 impl → trait_names 只出现一次
- first-seen 顺序保留（Drop, Clone, Add — 第三个 Drop 被去重）

### 2.5 计数字段

- incomplete_count: 有 missing 方法的 emission 数
- marker_count: marker emission 数
- complete_count: 完整 emission 数（含 vacuous markers）

### 2.6 结构体语义

- PartialEq/Eq 派生
- 从实际 stdlib_vtable_emission 构造的 summary 字段访问

## 3. 测试统计

- 新增: 13 tests
- 基线: 1223 tests
- 总计: 1236 tests
- 2 ignored (pre-existing, 未影响)

## 4. 依赖

- 上游: Stage 5.41 (`StdlibVtableEmission` + `stdlib_vtable_emission` +
  `stdlib_vtable_emissions_for_traits`)
- 下游:
  - Stage 5.43+ (codegen vtable emission refactor) — codegen 调用
    summary 输出诊断行
  - Stage 5.44+ (dyn Trait MIR lowering) — 同上

## 5. CI/CD 验证

```
cargo clean: clean (929.7 MiB removed)
cargo test: 1236 passed, 0 failed, 2 ignored ✅
cargo fmt --check: clean ✅
cargo clippy --all-targets: 0 warnings ✅ (修复了 1 个 cloned_ref_to_slice_refs 警告)
```

---

**创建日期**: 2026-07-23
