# Stage 5.94 开发计划：stdlib_trait_method 剩余字段访问器

> **阶段**: Stage 5.94
> **版本**: v0.11.89 → v0.11.90
> **状态**: ✅ Complete

## 1. 目标

添加 3 个剩余字段访问器，完成 `StdlibTraitMethod` 所有字段的便利访问器覆盖：
1. `stdlib_trait_method_self_kind(trait, method) -> Option<StdlibSelfKind>`
2. `stdlib_trait_method_param_count(trait, method) -> Option<u32>`
3. `stdlib_trait_method_is_unsafe(trait, method) -> Option<bool>`

Stage 5.93 添加了 return_kind + param_kinds。本 stage 添加剩余 3 个字段，
完成所有 6 个字段（name/self_kind/param_count/return_kind/param_kinds/is_unsafe）
的访问器覆盖。

## 2. 设计

### 2.1 新增 API

```rust
pub fn stdlib_trait_method_self_kind(trait_name: &str, method_name: &str) -> Option<StdlibSelfKind>
pub fn stdlib_trait_method_param_count(trait_name: &str, method_name: &str) -> Option<u32>
pub fn stdlib_trait_method_is_unsafe(trait_name: &str, method_name: &str) -> Option<bool>
```

### 2.2 命名标准化

| API | 命名规则 | 合规 |
|-----|---------|------|
| `stdlib_trait_method_self_kind` | `<noun>_<noun>_<noun>_<noun>_<noun>` | ✅ |
| `stdlib_trait_method_param_count` | `<noun>_<noun>_<noun>_<noun>_<noun>` | ✅ |
| `stdlib_trait_method_is_unsafe` | `<noun>_<noun>_<noun>_<noun>_<is_adj>` | ✅ |

参考 `stdlib_trait_method_return_kind` (5.93) 同家族。`is_unsafe` 遵循
`is_<adj>` 命名约定（§8.1）。

### 2.3 §16 接口隔离

- 输入：`&str` + `&str`
- 输出：`Option<T>`
- 纯只读，复用 `find_stdlib_trait_method`，无新依赖

### 2.4 测试矩阵

| # | 用例 | 期望 |
|---|------|------|
| 1 | Clone::clone self_kind == SelfByRef | ✓ |
| 2 | Drop::drop self_kind == SelfByMutRef | ✓ |
| 3 | Default::default self_kind == NoSelf | ✓ |
| 4 | Foo::bar self_kind == None | ✓ |
| 5 | Drop::drop param_count == 0 | ✓ |
| 6 | Display::fmt param_count == 1 | ✓ |
| 7 | Foo::bar param_count == None | ✓ |
| 8 | Drop::drop is_unsafe == false | ✓ |
| 9 | Foo::bar is_unsafe == None | ✓ |
| 10 | 与 find_stdlib_trait_method 一致性 | ✓ |

## 3. 不在本 stage 范围

- ❌ name 字段访问器（name 是查询参数，不需要访问器）
- ❌ 用户自定义 trait 支持（TD-018）
- ❌ mir/lower 拆分（TD-011）

---

**创建日期**: 2026-07-24
**作者**: Super Z (main)
