# Stage 5.84 开发计划：dyn Trait 参数类型精化

> **阶段**: Stage 5.84
> **版本**: v0.11.79 → v0.11.80
> **状态**: ✅ Complete

## 1. 目标

精化 dyn Trait 方法调用的参数类型处理（与 5.82 的 return_kind 对称）。
当前 codegen_dyn_trait_call 用 `EmitType::I32` 作为所有参数的默认类型
（通过 `detect_operand_type` fallback）。Stage 5.84 通过：

1. 在 `StdlibTraitMethod` 添加 `param_kinds: &'static [StdlibTypeKind]` 字段
   （保持 `Copy` + `&'static` 设计）
2. 在 `DynTraitMethodCall` 添加 `param_kinds: Vec<StdlibTypeKind>` 字段
3. 在 `build_dyn_trait_method_calls_from_fat_ptrs` 从 `method.param_kinds`
   传入该字段
4. 在 `codegen_dyn_trait_call` 使用 `call_info.param_kinds` 为每个参数
   精确推断 EmitType

## 2. 设计

### 2.1 StdlibTraitMethod 字段扩展

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StdlibTraitMethod {
    pub name: &'static str,
    pub self_kind: StdlibSelfKind,
    pub param_count: u32,
    pub return_kind: StdlibTypeKind,
    pub param_kinds: &'static [StdlibTypeKind],  // Stage 5.84
    pub is_unsafe: bool,
}
```

`&'static [StdlibTypeKind]` 保持 `Copy` + `&'static`，向后兼容静态表设计。

### 2.2 DynTraitMethodCall 字段扩展

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynTraitMethodCall {
    pub trait_name: String,
    pub type_name: String,
    pub method_name: String,
    pub slot_index: u32,
    pub param_count: u32,
    pub return_kind: StdlibTypeKind,
    pub param_kinds: Vec<StdlibTypeKind>,  // Stage 5.84
}
```

`Vec<StdlibTypeKind>` 因为 DynTraitMethodCall 已经是 owned（String 字段）。

### 2.3 构造函数更新

`new()` 和 `from_fat_ptr()` 添加 `param_kinds: Vec<StdlibTypeKind>` 参数。
**Breaking change**——所有调用点更新（用脚本辅助）。

### 2.4 build_dyn_trait_method_calls_from_fat_ptrs 更新

```rust
calls.push(DynTraitMethodCall::from_fat_ptr(
    fp,
    method.name,
    slot_index,
    method.param_count,
    method.return_kind,
    method.param_kinds.to_vec(),  // Stage 5.84
));
```

### 2.5 codegen_dyn_trait_call 更新

```rust
// Stage 5.84: use param_kinds for precise arg types
let arg_pairs: Vec<(EmitType, EmitValue)> = args
    .iter()
    .enumerate()
    .map(|(i, a)| {
        // Skip self (index 0); explicit args start at index 1
        let ty = if i > 0 && i - 1 < call_info.param_kinds.len() {
            stdlib_type_kind_to_emit_type(call_info.param_kinds[i - 1])
        } else {
            detect_operand_type(mir, a, layouts).unwrap_or(EmitType::I32)
        };
        let val = codegen_operand(emitter, mir, a, interner, layouts);
        (ty, val)
    })
    .collect();
```

### 2.6 命名标准化

`param_kinds` 字段名遵循 `<noun>_<noun>` 复数模式（与 `return_kind` 对称）。
无新 free function——复用 5.82 的 `stdlib_type_kind_to_emit_type`。

### 2.7 §16 接口隔离

- `StdlibTraitMethod.param_kinds` 在 `stdlib` 中定义（静态表）
- `DynTraitMethodCall.param_kinds` 在 `mir::dyn_trait` 中定义
- 数据流：stdlib → mir::dyn_trait → codegen，单向无循环

### 2.8 测试矩阵

| # | 用例 | 期望 |
|---|------|------|
| 1 | StdlibTraitMethod 有 param_kinds 字段 | ✓ |
| 2 | Clone::clone param_kinds = [] (无参数) | ✓ |
| 3 | Clone::clone_from param_kinds = [AllocType] (源 Self) | ✓ |
| 4 | DynTraitMethodCall::new 包含 param_kinds | ✓ |
| 5 | from_fat_ptr 包含 param_kinds | ✓ |
| 6 | build_dyn_trait_method_calls_from_fat_ptrs 传入 param_kinds | ✓ |
| 7 | codegen_dyn_trait_call 用 param_kinds 推断 arg 类型 | ✓ |
| 8 | 无参数方法 → IR 只含 self | ✓ |
| 9 | 有参数方法 → IR 含正确参数类型 | ✓ |
| 10 | 现有测试更新后通过 | ✓ |

## 3. 不在本 stage 范围

- ❌ 用户自定义 trait 的参数类型（仅 stdlib traits）
- ❌ self 参数类型精化（self 始终是 fat pointer）
- ❌ mir/lower 拆分（TD-011, Stage 6）

---

**创建日期**: 2026-07-24
**作者**: Super Z (main)
