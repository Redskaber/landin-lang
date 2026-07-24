# Stage 5.93 开发计划：stdlib_trait_method_return_kind + stdlib_trait_method_param_kinds 便利访问器

> **阶段**: Stage 5.93
> **版本**: v0.11.88 → v0.11.89
> **状态**: ✅ Complete

## 1. 目标

添加两个便利访问器函数：
1. `stdlib_trait_method_return_kind(trait, method) -> Option<StdlibTypeKind>` —— 直接查询方法返回类型
2. `stdlib_trait_method_param_kinds(trait, method) -> Option<&'static [StdlibTypeKind]>` —— 直接查询方法参数类型列表

当前需要 `find_stdlib_trait_method(trait, method)?.return_kind` 两步访问。
本 stage 提供一步访问，与 `stdlib_trait_method_count` / `stdlib_trait_method_index`
同家族。

## 2. 设计动机

当前 stdlib trait method 查询函数：
- `stdlib_trait_method_count(trait) -> Option<usize>` —— 方法数
- `stdlib_trait_method_index(trait, method) -> Option<u32>` —— slot index
- `find_stdlib_trait_method(trait, method) -> Option<&StdlibTraitMethod>` —— 完整 struct

**缺失**：直接查询 return_kind / param_kinds 的便利函数。当前需要：
```rust
let kind = find_stdlib_trait_method(trait, method)?.return_kind;
```

新函数提供一步访问：
```rust
let kind = stdlib_trait_method_return_kind(trait, method)?;
```

## 3. 设计

### 3.1 新增 API

```rust
/// Stage 5.93: Get the return type kind of a stdlib trait method.
pub fn stdlib_trait_method_return_kind(
    trait_name: &str,
    method_name: &str,
) -> Option<StdlibTypeKind>

/// Stage 5.93: Get the parameter type kinds of a stdlib trait method.
pub fn stdlib_trait_method_param_kinds(
    trait_name: &str,
    method_name: &str,
) -> Option<&'static [StdlibTypeKind]>
```

### 3.2 计算规则

```rust
pub fn stdlib_trait_method_return_kind(trait_name: &str, method_name: &str) -> Option<StdlibTypeKind> {
    find_stdlib_trait_method(trait_name, method_name).map(|m| m.return_kind)
}

pub fn stdlib_trait_method_param_kinds(trait_name: &str, method_name: &str) -> Option<&'static [StdlibTypeKind]> {
    find_stdlib_trait_method(trait_name, method_name).map(|m| m.param_kinds)
}
```

### 3.3 命名标准化

| API | 命名规则 | 合规 |
|-----|---------|------|
| `stdlib_trait_method_return_kind` | `<noun>_<noun>_<noun>_<noun>_<noun>` | ✅ |
| `stdlib_trait_method_param_kinds` | `<noun>_<noun>_<noun>_<noun>_<noun>` (plural) | ✅ |

参考 `stdlib_trait_method_count` / `stdlib_trait_method_index` 同家族——
`stdlib_trait_method_<field>` 模式。

### 3.4 §16 接口隔离

- 输入：`&str` + `&str`
- 输出：`Option<StdlibTypeKind>` / `Option<&'static [StdlibTypeKind]>`
- 纯只读，复用 `find_stdlib_trait_method`，无新依赖

### 3.5 测试矩阵

| # | 用例 | 期望 |
|---|------|------|
| 1 | Drop::drop return_kind == Unit | ✓ |
| 2 | Clone::clone return_kind == AllocType | ✓ |
| 3 | Display::fmt return_kind == StdType | ✓ |
| 4 | PartialEq::eq return_kind == Bool | ✓ |
| 5 | Foo::bar return_kind == None (not in stdlib) | ✓ |
| 6 | Drop::drop param_kinds == [] (empty) | ✓ |
| 7 | Display::fmt param_kinds == [StdType] | ✓ |
| 8 | Clone::clone_from param_kinds == [AllocType] | ✓ |
| 9 | Foo::bar param_kinds == None (not in stdlib) | ✓ |
| 10 | 与 find_stdlib_trait_method 一致性 | ✓ |

## 4. 不在本 stage 范围

- ❌ 其他字段访问器（self_kind/is_unsafe 等留待需要时添加）
- ❌ 用户自定义 trait 支持（TD-018）
- ❌ mir/lower 拆分（TD-011）

---

**创建日期**: 2026-07-24
**作者**: Super Z (main)
