# Stage 5.42 开发计划：stdlib vtable emission summary + 深度审查 #4

> **阶段**: Stage 5.42
> **版本**: v0.11.37 → v0.11.38
> **状态**: ✅ Complete

## 1. 目标

在 Stage 5.41（单 vtable emission 聚合）基础上，添加**项目级 vtable emission
摘要**：给定一组 (trait, type, provided) 三元组，返回整个项目的 vtable emission
统计信息——总 emission 数、marker 数、complete 数、incomplete 数、总 slot 数、
32/64 位总字节大小。这是 dyn Trait codegen 修改前的最后一步静态分析，也是
**触发 §25 深度审查 #4** 的里程碑（Stage 5.36-5.42 共 7 个子阶段，到达
深度审查频率阈值）。

## 2. 设计

### 2.1 新增类型

```rust
/// 项目级 vtable emission 统计摘要。
pub struct StdlibVtableEmissionSummary {
    pub total_emissions: u32,        // 总 emission 数
    pub marker_count: u32,           // marker trait emission 数
    pub complete_count: u32,         // 完整（所有 slot provided）的 emission 数
    pub incomplete_count: u32,       // 不完整的 emission 数
    pub total_slots: u32,            // 所有 emission 的 slot 总数
    pub total_byte_size_32: u64,     // 32-bit 总字节大小
    pub total_byte_size_64: u64,     // 64-bit 总字节大小
    pub trait_names: Vec<&'static str>, // 涉及的 trait 名（去重）
}
```

### 2.2 新增 API

| API | 签名 | 用途 |
|-----|------|------|
| `stdlib_vtable_emission_summary` | `(&[StdlibVtableEmission]) -> StdlibVtableEmissionSummary` | 聚合统计 |

### 2.3 计算规则

- `total_emissions` = 输入 emissions 长度
- `marker_count` = `is_marker == true` 的数量
- `complete_count` = `is_complete == true` 的数量
- `incomplete_count` = `total_emissions - complete_count`
- `total_slots` = 所有 emission 的 `slot_count` 之和
- `total_byte_size_32` = 所有 `byte_size_32` 之和
- `total_byte_size_64` = 所有 `byte_size_64` 之和
- `trait_names` = 所有 `trait_name` 去重后的列表

### 2.4 命名标准化（§23）

| API/类型 | 命名规则 | 合规 |
|----------|---------|------|
| `StdlibVtableEmissionSummary` | `<Noun><Noun><Noun><Noun>` | ✅ |
| `stdlib_vtable_emission_summary` | `<noun>_<noun>_<noun>_<noun>` | ✅ |
| `total_emissions` / `marker_count` / `complete_count` / `incomplete_count` / `total_slots` / `total_byte_size_32` / `total_byte_size_64` / `trait_names` | fields | ✅ |

### 2.5 §16 接口隔离

`StdlibVtableEmissionSummary` 仅依赖 `&'static str` + `Vec<>` + 标量字段，
不引用 `codegen::EmitType` / `mir::ty` / `traits::TraitResolver`，无循环依赖。

## 3. 验收标准

1. `cargo fmt --check` 零 diff
2. `cargo clippy --all-targets` 0 warnings
3. `cargo test` 全部通过（基线 1223 + 新增 ~12 = ~1235）
4. §1.2 交付前验收：全绿
5. §25 深度审查 #4 完成（7 维度分析）

## 4. 测试矩阵

| 测试 | 描述 |
|------|------|
| `test_stdlib_vtable_emission_summary_empty` | 空输入 → 全 0 |
| `test_stdlib_vtable_emission_summary_single_complete` | 单个完整 emission |
| `test_stdlib_vtable_emission_summary_single_marker` | 单个 marker |
| `test_stdlib_vtable_emission_summary_multi_mixed` | 多个混合（complete + incomplete + marker） |
| `test_stdlib_vtable_emission_summary_total_slots` | slot 总数累加 |
| `test_stdlib_vtable_emission_summary_byte_sizes` | 32/64 位字节大小累加 |
| `test_stdlib_vtable_emission_summary_trait_names_dedup` | trait 名去重 |
| `test_stdlib_vtable_emission_summary_incomplete_count` | 不完整计数 |
| `test_stdlib_vtable_emission_summary_marker_count` | marker 计数 |
| `test_stdlib_vtable_emission_summary_complete_count` | 完整计数 |
| `test_stdlib_vtable_emission_summary_struct_eq` | PartialEq/Eq 派生 |
| `test_stdlib_vtable_emission_summary_from_real_emissions` | 从实际 emission 构造 |

## 5. §25 深度审查 #4 触发条件

Stage 5.32 完成深度审查 #3（r81）。自此后完成 10 个子阶段（5.33-5.42），
到达 §25.6 "每 10 个子阶段或重大里程碑触发深度审查" 的频率阈值。本轮
将在交付前完成 §25 7 维度审查并写入 `deep-review-r91.md`。

## 6. 后续依赖

- **Stage 5.43+ (codegen vtable emission refactor)**: codegen 调用
  `stdlib_vtable_emissions_for_traits()` 获取 emission 列表，再调用
  `stdlib_vtable_emission_summary()` 输出诊断信息（"emit N vtables, M
  bytes total"）。
- **Stage 5.44+ (dyn Trait MIR lowering)**: 同上。

---

**创建日期**: 2026-07-23
