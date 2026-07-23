# Stage 5.34 开发计划：stdlib type resolution

> **阶段**: Stage 5.34
> **版本**: v0.11.29 → v0.11.30
> **状态**: ✅ Complete

## 1. 目标

添加 `StdlibTypeKind` 枚举 + `resolve_stdlib_type()` + 类型查询函数，
支持 stdlib 类型名 → kind 映射（避免 mir::ty 循环依赖）。

## 2. 设计

### 2.1 新增 API

| API | 签名 | 用途 |
|-----|------|------|
| `StdlibTypeKind` | enum | 类型 kind 枚举（I8-I128/U8-U128/F32/F64/Bool/Char/Str/Unit/Never/AllocType/StdType/Unknown） |
| `resolve_stdlib_type` | `(name: &str) -> StdlibTypeKind` | 类型名 → kind |
| `is_primitive_type` | `(name: &str) -> bool` | 是否原始类型 |
| `integer_bit_width` | `(name: &str) -> Option<u32>` | 整数位宽 |
| `is_signed_integer` | `(name: &str) -> bool` | 有符号整数 |
| `is_unsigned_integer` | `(name: &str) -> bool` | 无符号整数 |
| `is_float_type` | `(name: &str) -> bool` | 浮点类型 |

### 2.3 命名标准化

| API | 命名规则 |
|-----|---------|
| `StdlibTypeKind` | `<Noun><Noun>` |
| `resolve_stdlib_type` | `resolve_<noun>_<noun>` |
| `is_primitive_type` | `is_<adj>_<noun>` |
| `integer_bit_width` | `<noun>_<noun>` |
| `is_signed_integer` | `is_<adj>_<noun>` |
| `is_unsigned_integer` | `is_<adj>_<noun>` |
| `is_float_type` | `is_<adj>_<noun>` |

## 3. 验收标准

1. `cargo fmt --check` 零 diff ✅
2. `cargo clippy --all-targets` 0 warnings ✅
3. `cargo test` 全部通过 ✅
4. §1.2 交付前验收：全绿 ✅

---

**创建日期**: 2026-07-23
