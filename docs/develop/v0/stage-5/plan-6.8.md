# Stage 6.8 开发计划：codegen/mod.rs 架构性拆分 — 类型翻译 + place/operand 辅助提取

> **阶段**: Stage 6.8
> **版本**: v0.12.6 → v0.12.7
> **状态**: 🟡 In Progress

## 1. 目标

继续 codegen/mod.rs 的架构性拆分（TD-017 第二步）。

**架构原则**（用户强调）：
- 不是单纯缩小体积，而是科学合理划分模块边界
- 符合架构设计需求，本质上是组织结构设计

## 2. 当前 codegen/mod.rs（1512 LOC）职责分析

Stage 6.7 提取了 trait_dispatch 域。剩余 mod.rs 包含两个内聚性不同的域：

### 域 A: MIR → LLVM IR 翻译核心（~1100 LOC）
入口点 + 语句/表达式/终止符翻译 + dyn trait call
- `codegen_crate` / `codegen_from_mir` / `codegen_function`
- `codegen_statement` / `codegen_rvalue` / `codegen_operand` / `codegen_terminator`
- `codegen_dyn_trait_call`
→ **这是 codegen 的核心翻译逻辑，是 mod.rs 应该保留的内容**

### 域 B: 类型翻译 + place/operand 辅助（~400 LOC）
类型转换 + place 地址计算 + 类型检测
- `mir_type_to_emit_type_with_layouts` / `stdlib_type_kind_to_emit_type`
- `detect_place_type` / `detect_place_storage_type` / `detect_operand_type`
- `compute_place_address` / `unwrap_fat_ptr_for_index`
- `codegen_place_load_typed` / `codegen_place_load`
→ **这是翻译辅助层：MIR类型→EmitType转换 + Place地址计算 + 类型检测**

## 3. 架构设计方案

提取域 B 到 `codegen/mir_translation.rs`——**MIR 类型/Place/Operand 到 EmitType/EmitValue 的翻译辅助层**。

**设计理由**：
- 域 B 有清晰的内聚性：所有函数都是"MIR 数据结构 → codegen 辅助数据"的桥接
- `mir_type_to_emit_type_with_layouts` 是 §8.2 定义的翻译阶梯第一步（MIR Ty → EmitType）
- `stdlib_type_kind_to_emit_type` 是 stdlib → codegen 的类型桥接
- `detect_*` / `compute_*` 是 MIR Place/Operand 的 introspection 辅助
- `codegen_place_load*` 是 Place 加载的通用辅助（被 statement/rvalue/terminator 共用）

**模块边界**：
- `mod.rs` = MIR body → LLVM IR 翻译核心（codegen_statement/rvalue/operand/terminator）
- `trait_dispatch.rs` = TraitResolver → vtable/dynptr 全局（Stage 6.7）
- `mir_translation.rs` = MIR 类型/Place/Operand 翻译辅助（本 stage）
- `emitter.rs` = Emitter trait + EmitType/EmitValue 定义（已有）
- `text_emitter.rs` = TextEmitter impl（已有）

## 4. 提取的函数

| 函数 | LOC | 职责 |
|------|-----|------|
| `mir_type_to_emit_type_with_layouts` | ~105 | MIR Ty → EmitType (with ADT layouts) |
| `stdlib_type_kind_to_emit_type` | ~23 | StdlibTypeKind → EmitType |
| `detect_place_storage_type` | ~30 | 检测 Place 的存储类型 |
| `detect_place_type` | ~55 | 检测 Place 的 EmitType |
| `compute_place_address` | ~45 | 计算 Place 的 LLVM 地址 |
| `unwrap_fat_ptr_for_index` | ~25 | 解包 fat pointer 用于索引 |
| `codegen_place_load_typed` | ~100 | 类型化 Place 加载 |
| `codegen_place_load` | ~55 | Place 加载 |
| `detect_operand_type` | ~80 | 检测 Operand 的 EmitType |

**预计提取 ~520 LOC**

---

**创建日期**: 2026-07-24
