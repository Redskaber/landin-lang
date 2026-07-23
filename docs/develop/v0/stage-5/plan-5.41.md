# Stage 5.41 开发计划：stdlib vtable emission plan

> **阶段**: Stage 5.41
> **版本**: v0.11.36 → v0.11.37
> **状态**: ✅ Complete

## 1. 目标

在 Stage 5.36-5.40 基础上，添加**vtable emission 聚合结构**：一次调用
返回 codegen emit `@.vtable.<trait>.<type>` 全局所需的**全部信息**——
全局名 + 方法符号列表 + 字节大小 + slot count。这是 codegen 修改前的
"最终聚合 API"——Stage 5.42+ 修改 codegen 时只需调用
`stdlib_vtable_emission()` 一次，获取 `StdlibVtableEmission` 结构体，
不再分别调用 5 个不同的 stdlib 函数。

## 2. 设计

### 2.1 新增类型

```rust
/// codegen emit 一个 vtable 全局所需的全部信息。
pub struct StdlibVtableEmission {
    pub trait_name: &'static str,
    pub type_name: String,
    pub global_name: String,           // ".vtable.<trait>.<type>"
    pub method_symbols: Vec<String>,   // ["landin_T_m1", "null", ...]
    pub slot_count: u32,
    pub byte_size_32: u64,             // 32-bit target
    pub byte_size_64: u64,             // 64-bit target
    pub is_marker: bool,               // true if slot_count == 0
    pub is_complete: bool,             // true if all slots provided
}
```

### 2.2 新增 API

| API | 签名 | 用途 |
|-----|------|------|
| `stdlib_vtable_emission` | `(trait, type, provided, ...) -> Option<StdlibVtableEmission>` | 单次调用聚合全部 emission 信息 |
| `stdlib_vtable_emissions_for_traits` | `(traits: &[&str], type, provided, ...) -> Vec<StdlibVtableEmission>` | 批量 emission（每 trait 一个） |

### 2.3 计算规则

- `global_name` = `stdlib_vtable_global_name(trait, type)`
- `method_symbols` = `stdlib_vtable_method_symbols(trait, type, provided)?`
- `slot_count` = `method_symbols.len() as u32`
- `byte_size_32` = `slot_count × 4`
- `byte_size_64` = `slot_count × 8`
- `is_marker` = `slot_count == 0`
- `is_complete` = `!method_symbols.contains(&"null".to_string())`
- 未注册 trait → `None`

### 2.4 命名标准化（§23）

| API/类型 | 命名规则 | 合规 |
|----------|---------|------|
| `StdlibVtableEmission` | `<Noun><Noun><Noun>` | ✅ |
| `stdlib_vtable_emission` | `<noun>_<noun>_<noun>` | ✅ |
| `stdlib_vtable_emissions_for_traits` | `<noun>_<noun>_<noun>_<prep>_<noun>` | ✅ |
| `trait_name` / `type_name` / `global_name` / `method_symbols` / `slot_count` / `byte_size_32` / `byte_size_64` / `is_marker` / `is_complete` | fields | ✅ |

### 2.5 §16 接口隔离

`StdlibVtableEmission` 仅依赖 `&'static str` + `String` + `Vec<String>` +
标量字段，不引用 `codegen::EmitType` / `mir::ty` / `traits::TraitResolver`，
无循环依赖。所有查询函数是纯函数。

## 3. 验收标准

1. `cargo fmt --check` 零 diff
2. `cargo clippy --all-targets` 0 warnings
3. `cargo test` 全部通过（基线 1206 + 新增 ~14 = ~1220）
4. §1.2 交付前验收：全绿

## 4. 测试矩阵

| 测试 | 描述 |
|------|------|
| `test_stdlib_vtable_emission_clone_complete` | Clone + S + [clone, clone_from] → 2 slots, complete |
| `test_stdlib_vtable_emission_clone_partial` | Clone + S + [clone] → 2 slots, not complete |
| `test_stdlib_vtable_emission_drop` | Drop + S + [drop] → 1 slot |
| `test_stdlib_vtable_emission_marker` | Copy + S + [] → 0 slots, is_marker=true |
| `test_stdlib_vtable_emission_unknown_trait` | BogusTrait → None |
| `test_stdlib_vtable_emission_global_name` | global_name 字段正确 |
| `test_stdlib_vtable_emission_byte_sizes` | byte_size_32 / byte_size_64 正确 |
| `test_stdlib_vtable_emission_is_complete_true` | 完整 → is_complete=true |
| `test_stdlib_vtable_emission_is_complete_false` | 部分 → is_complete=false |
| `test_stdlib_vtable_emission_is_marker` | Copy → is_marker=true; Clone → false |
| `test_stdlib_vtable_emission_arith` | Add + Vec + [add] → 1 slot |
| `test_stdlib_vtable_emissions_for_traits` | 批量：Clone + Drop → 2 emissions |
| `test_stdlib_vtable_emissions_for_traits_filters_unknown` | 未知 trait 静默跳过 |
| `test_stdlib_vtable_emission_struct_eq` | PartialEq/Eq 派生 |

## 5. 后续依赖

- **Stage 5.42+ (codegen vtable emission refactor)**: codegen 调用
  `stdlib_vtable_emission()` 一次，直接消费 `StdlibVtableEmission` 字段
  生成 LLVM IR——无需分别调用 5 个 stdlib 函数，代码更简洁。
- **Stage 5.43+ (dyn Trait MIR lowering)**: MIR lowering 调用
  `stdlib_vtable_emissions_for_traits()` 批量获取所有需要 emit 的 vtable。

---

**创建日期**: 2026-07-23
