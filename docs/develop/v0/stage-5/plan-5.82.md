# Stage 5.82 开发计划：TD-016 — dyn Trait return type 精化

> **阶段**: Stage 5.82
> **版本**: v0.11.77 → v0.11.78
> **状态**: ✅ Complete

## 1. 目标

精化 dyn Trait 方法调用的返回类型处理（TD-016）。当前 Stage 5.79 的
`codegen_dyn_trait_call` 用 `EmitType::I32` 作为 return type placeholder
（因为 MIR 不携带 typeck-resolved return type）。Stage 5.82 通过：

1. 在 `DynTraitMethodCall` 添加 `return_kind: StdlibTypeKind` 字段
2. 在 `build_dyn_trait_method_calls_from_fat_ptrs` 从 `StdlibTraitMethod.return_kind`
   传入该字段
3. 在 codegen 添加 `stdlib_type_kind_to_emit_type()` 转换函数
4. 在 `codegen_dyn_trait_call` 使用 `call_info.return_kind` 转换为 `EmitType`

这让 codegen 生成精确类型的 vtable indirect call IR，而非所有调用都用 i32。

## 2. 设计

### 2.1 DynTraitMethodCall 字段扩展

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynTraitMethodCall {
    pub trait_name: String,
    pub type_name: String,
    pub method_name: String,
    pub slot_index: u32,
    pub param_count: u32,
    /// Stage 5.82: Return type kind (from StdlibTraitMethod.return_kind).
    /// Used by codegen to emit the correct LLVM return type instead of
    /// the I32 placeholder. Defaults to Unit for methods with no return.
    pub return_kind: crate::stdlib::StdlibTypeKind,
}
```

### 2.2 构造函数更新

`new()` 和 `from_fat_ptr()` 添加 `return_kind` 参数。**Breaking change**
for existing callers——所有现有测试需要更新。

为了减小 breakage，添加 `new_with_return_kind()` 显式版本，并保留 `new()`
使用 `Unit` 默认值（向后兼容）。

实际上，由于 DynTraitMethodCall 主要由 `build_dyn_trait_method_calls_from_fat_ptrs`
内部构造，外部测试直接构造的情况有限。我们采用**直接更新 new() 签名**的方案，
更新所有调用点。

### 2.3 build_dyn_trait_method_calls_from_fat_ptrs 更新

```rust
calls.push(DynTraitMethodCall::from_fat_ptr(
    fp,
    method.name,
    slot_index,
    method.param_count,
    method.return_kind,  // Stage 5.82: pass return_kind
));
```

### 2.4 stdlib_type_kind_to_emit_type() 转换函数

```rust
/// Stage 5.82: Convert StdlibTypeKind to EmitType for codegen.
pub fn stdlib_type_kind_to_emit_type(kind: StdlibTypeKind) -> EmitType {
    match kind {
        StdlibTypeKind::I8 | StdlibTypeKind::U8 | StdlibTypeKind::Bool | StdlibTypeKind::Char => EmitType::I8,
        StdlibTypeKind::I16 | StdlibTypeKind::U16 => EmitType::I16,
        StdlibTypeKind::I32 | StdlibTypeKind::U32 => EmitType::I32,
        StdlibTypeKind::I64 | StdlibTypeKind::U64 => EmitType::I64,
        StdlibTypeKind::I128 | StdlibTypeKind::U128 => EmitType::I128,
        StdlibTypeKind::F32 => EmitType::F32,
        StdlibTypeKind::F64 => EmitType::F64,
        StdlibTypeKind::Unit | StdlibTypeKind::Never => EmitType::Void,
        // AllocType/StdType/Str/Unknown → opaque pointer (dyn Trait receivers
        // are fat pointers; method returns of these types are ptr-sized)
        StdlibTypeKind::AllocType | StdlibTypeKind::StdType | StdlibTypeKind::Str | StdlibTypeKind::Unknown => EmitType::OpaquePtr,
    }
}
```

### 2.5 codegen_dyn_trait_call 更新

```rust
// Stage 5.82: use return_kind for precise return type (was I32 placeholder)
let ret_ty = stdlib_type_kind_to_emit_type(call_info.return_kind);
emitter.emit_dyn_trait_method_call(&dynptr_symbol, call_info.slot_index, &arg_refs, &ret_ty)
```

### 2.6 命名标准化

| API | 命名规则 | 合规 |
|-----|---------|------|
| `stdlib_type_kind_to_emit_type` | `<noun>_<noun>_<noun>_<prep>_<noun>_<noun>` | ✅ |

参考 §8.2 translation function ladder——`mir_type_to_emit_type` / `emit_type_to_llvm_str`
同家族的"X to Y"转换函数。

### 2.7 §16 接口隔离

- `stdlib_type_kind_to_emit_type` 在 `codegen::mod` 中定义
- 输入：`StdlibTypeKind`（来自 `stdlib`）
- 输出：`EmitType`（codegen 内部）
- 数据流：stdlib → codegen 单向，无循环依赖

### 2.8 测试矩阵

| # | 用例 | 期望 |
|---|------|------|
| 1 | DynTraitMethodCall::new 包含 return_kind 字段 | ✓ |
| 2 | return_kind 默认值合理（Unit for void methods） | ✓ |
| 3 | build_dyn_trait_method_calls_from_fat_ptrs 传入 return_kind | ✓ |
| 4 | stdlib_type_kind_to_emit_type: I32 → EmitType::I32 | ✓ |
| 5 | stdlib_type_kind_to_emit_type: Bool → EmitType::I8 | ✓ |
| 6 | stdlib_type_kind_to_emit_type: Unit → EmitType::Void | ✓ |
| 7 | stdlib_type_kind_to_emit_type: F64 → EmitType::F64 | ✓ |
| 8 | stdlib_type_kind_to_emit_type: AllocType → OpaquePtr | ✓ |
| 9 | codegen_dyn_trait_call 使用 return_kind 而非 I32 | IR 含正确 ret type |
| 10 | Drop::drop (return Unit) → call void | ✓ |
| 11 | Clone::clone (return Self=AllocType) → call ptr | ✓ |
| 12 | 现有测试更新后通过 | ✓ |

## 3. 不在本 stage 范围

- ❌ 用户自定义 trait 的 return type（仅 stdlib traits 有 return_kind）
- ❌ dyn Trait 参数类型精化（仅 return type）
- ❌ mir/lower 拆分（TD-011, Stage 6）

---

**创建日期**: 2026-07-24
**作者**: Super Z (main)
