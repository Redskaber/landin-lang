# Stage 6.7 开发计划：codegen/mod.rs 架构性拆分 — trait dispatch emission 提取（TD-017 第一步）

> **阶段**: Stage 6.7
> **版本**: v0.12.5 → v0.12.6
> **状态**: 🟡 In Progress

## 1. 目标

开始偿还 TD-017（codegen/mod.rs 2461 LOC 拆分）。

**架构性拆分原则**（用户特别强调）：
- 不是单纯缩小体积，而是**科学合理划分模块边界**
- 按职责内聚性分组，符合架构设计需求
- 本质上是组织结构设计

## 2. 架构分析

当前 `codegen/mod.rs`（2461 LOC）包含 **3 个不同的职责域**：

### 域 1: MIR → LLVM IR 翻译核心（~1500 LOC）
- `codegen_crate` / `codegen_from_mir` / `codegen_function`
- `codegen_statement` / `codegen_rvalue` / `codegen_operand` / `codegen_terminator`
- `codegen_place_load` / `codegen_place_load_typed` / `compute_place_address`
- `mir_type_to_emit_type_with_layouts` / `stdlib_type_kind_to_emit_type`
- `detect_place_type` / `detect_operand_type` / `detect_place_storage_type`
- `codegen_dyn_trait_call` / `unwrap_fat_ptr_for_index`
→ 这些是 **MIR 消费者**，是 codegen 的核心翻译逻辑

### 域 2: Vtable/Dynptr 全局变量生成（~850 LOC）
- `emit_vtables` / `emit_dyn_trait_ptrs`
- `emit_vtable_global_from_emission` / `emit_vtable_global_text` / `emit_vtable_globals_batch`
- `build_vtable_global_specs` / `emit_vtables_from_resolver`
- `emit_dynptr_global_text` / `build_dynptr_global_specs` / `emit_dynptrs_from_resolver`
- `emit_vtables_and_dynptrs_from_resolver`
→ 这些是 **TraitResolver 消费者**，生成 `@.vtable.*` / `@.dynptr.*` 全局变量

### 域 3: Trait dispatch emission 编排（~400 LOC）
- `build_trait_dispatch_emission_summary` / `build_trait_dispatch_emission_plan`
- `emit_trait_dispatch_globals_from_plan`
- `emit_trait_dispatch_globals_text_batch` / `emit_trait_dispatch_globals_text_batch_from_resolver`
→ 这些是 **高层编排 API**，组合域 1 和域 2

## 3. 拆分计划

### 第一步（本 stage）：提取域 2 + 域 3 → `codegen/trait_dispatch.rs`

将 vtable/dynptr 全局变量生成（域 2）+ trait dispatch 编排（域 3）
提取到 `codegen/trait_dispatch.rs`。这些函数的职责是：
"从 TraitResolver 生成 vtable/dynptr 全局变量"——一个清晰的内聚模块。

### 后续步骤（Stage 6.8+）：
- 域 1 的 type translation 可以提取到 `codegen/type_translation.rs`
- 域 1 的 place/operand 辅助可以提取到 `codegen/place_codegen.rs`

## 4. 提取的函数（域 2 + 域 3）

| 函数 | LOC | 职责 |
|------|-----|------|
| `emit_vtables` | 30 | 从 TraitResolver 发射所有 vtable 全局 |
| `emit_dyn_trait_ptrs` | 15 | 从 TraitResolver 发射所有 dynptr 全局 |
| `emit_vtable_global_from_emission` | 60 | 从 StdlibVtableEmission 生成 vtable IR |
| `emit_vtable_global_text` | 80 | 生成 vtable 全局 IR 文本 |
| `emit_vtable_globals_batch` | 40 | 批量生成 vtable 全局 |
| `build_vtable_global_specs` | 60 | 构建 vtable 全局规格 |
| `emit_vtables_from_resolver` | 60 | 从 resolver 发射 vtable |
| `emit_dynptr_global_text` | 80 | 生成 dynptr 全局 IR 文本 |
| `build_dynptr_global_specs` | 70 | 构建 dynptr 全局规格 |
| `emit_dynptrs_from_resolver` | 50 | 从 resolver 发射 dynptr |
| `emit_vtables_and_dynptrs_from_resolver` | 75 | 合并发射 vtable + dynptr |
| `build_trait_dispatch_emission_summary` | 100 | 构建 emission summary |
| `build_trait_dispatch_emission_plan` | 50 | 构建 emission plan |
| `emit_trait_dispatch_globals_from_plan` | 55 | 从 plan 发射全局 |
| `emit_trait_dispatch_globals_text_batch` | 60 | 批量发射全局文本 |
| `emit_trait_dispatch_globals_text_batch_from_resolver` | 65 | 从 resolver 批量发射 |

**预计提取 ~850 LOC**

---

**创建日期**: 2026-07-24
