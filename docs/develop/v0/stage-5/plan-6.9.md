# Stage 6.9 开发计划：stdlib.rs 架构性拆分 — 3 域分离

> **阶段**: Stage 6.9
> **版本**: v0.12.7 → v0.12.8
> **状态**: 🟡 In Progress

## 1. 目标

对 `stdlib.rs`（2383 LOC）进行架构性拆分。遵循单一职责原则，
按数据域和查询域科学划分。

**架构原则**（用户强调）：
- 不是单纯缩小体积，而是科学合理划分模块边界
- 符合架构设计需求，本质上是组织结构设计

## 2. 架构分析

`stdlib.rs` 当前包含 **3 个不同的职责域**：

### 域 A: 类型系统 + 预注册（~600 LOC, lines 1-585）
- 常量定义（STDLIB_CORE_TYPES, STDLIB_ALLOC_TYPES, ...）
- StdlibPrelude / StdlibFacade / StdlibLayer
- register_stdlib / default_prelude
- StdlibTypeKind + resolve_stdlib_type
- 类型查询：is_primitive_type, integer_bit_width, is_signed_integer, ...
- 类型布局：type_size_bytes, type_alignment_bytes, is_zero_sized_type, type_description
- **职责**：定义 stdlib 的类型世界 + 预注册到 interner

### 域 B: Trait 方法签名 + 查询（~900 LOC, lines 587-1680）
- StdlibSelfKind + StdlibTraitMethod
- 静态方法表（CLONE_METHODS, DROP_METHODS, ...）
- 正向查询：stdlib_trait_methods, find_stdlib_trait_method, ...
- 字段访问器：stdlib_trait_method_return_kind, ...
- 反向查询：stdlib_trait_methods_by_self_kind, ...
- 语义分组：stdlib_marker_traits, stdlib_arithmetic_traits, ...
- STDLIB_TRAITS 常量 + 成员查询：is_stdlib_trait, stdlib_trait_count, stdlib_all_traits
- **职责**：定义 stdlib trait 方法签名 + 提供查询 API

### 域 C: Vtable 布局 + 符号 + Emission（~780 LOC, lines 1680-2383）
- StdlibPointerWidth + stdlib_pointer_width_bytes
- StdlibVtableSlot + stdlib_vtable_layout + stdlib_vtable_slot_count
- stdlib_vtable_byte_size + stdlib_vtable_method_offset
- StdlibVtablePlan + stdlib_vtable_plan + ...
- 符号生成：stdlib_vtable_global_name, stdlib_dynptr_global_name, ...
- StdlibVtableEmission + stdlib_vtable_emission + ...
- StdlibVtableEmissionSummary
- **职责**：vtable 内存布局规划 + LLVM 符号生成 + emission 聚合

## 3. 拆分方案

```
src/stdlib/
  mod.rs          — re-exports + 域 A (类型系统 + 预注册)
  trait_methods.rs — 域 B (trait 方法签名 + 查询)
  vtable_layout.rs — 域 C (vtable 布局 + 符号 + emission)
```

**设计理由**：
- 域 A 是基础——定义了类型世界，其他域依赖它
- 域 B 依赖域 A（StdlibTypeKind 用于 return_kind/param_kinds）
- 域 C 依赖域 A + 域 B（vtable 布局需要 trait 方法 slot 信息）
- 数据流单向：types → trait_methods → vtable_layout，无循环

---

**创建日期**: 2026-07-24
