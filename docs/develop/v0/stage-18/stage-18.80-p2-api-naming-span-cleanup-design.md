# Stage 18.80 — P2 API Naming + Span::DUMMY Cleanup

> **Author**: redskaber
> **Date**: 2026-08-09
> **Version**: v0.347.0 → v0.348.0
> **Process**: stage-committee-process.md v5.0 §13.1 (设计对齐) + §13.5 (设计-审查) + §14 (深度审查)
> **Status**: ✅ Complete

## 1. 背景

Stage 18.79 完成了 P2 测试体系清理。本 Stage 推进 P2 API 命名标准化
和 Span::DUMMY 清理。

## 2. P2 修复项

| P2 # | 描述 | 修复方案 |
|------|------|---------|
| P2-A | 11 处 `get_` 前缀函数 | 重命名为无前缀 (Rust 惯例) |
| P2-B | 6 处名词访问器方法 | 重命名为 `find_*` 或 `*_for` |
| P2-C | ~30 处 `pub fn` 应为 `pub(crate)` | 降级可见性 |
| P2-D | 14 处 HIGH 优先级 Span::DUMMY | 替换为真实 span |

### 2.1 不修复项 (需更大重构)

- **5 个错误类型 Kind enum**: 需要 API 设计, 延后到独立 stage
- **TraitError 位置迁移**: 涉及大量 import 变更, 延后
- **format_for_user 遗留名**: 已 deprecated, 保留

## 3. 设计方案

### 3.1 §1.0 原则应用

| 原则 | 应用 |
|------|------|
| 3 显式 > 隐式 | span 显式传递, 不用 DUMMY |
| 7 API 命名标准化 | 遵循 Rust 命名惯例 |
| 9 正确 > 妥协 | 不保留违反惯例的命名 |

### 3.2 P2-A: get_ 前缀移除

Rust 惯例: 属性访问器不加 `get_` 前缀 (用 `name()` 而非 `get_name()`)。

| 当前 | 修复后 |
|------|--------|
| `get_local` | `local` |
| `get_local_ptr` | `local_ptr` |
| `get_struct_fields` | `struct_fields` |
| `get_const_val` | `const_val` |
| `get_const_ty` | `const_ty` |
| `get_or_declare_function` | `declare_function` |
| `get_call_dest_type` | `call_dest_type` |

**策略**: 添加 `#[deprecated]` 别名保持兼容性, 新代码使用新名。

### 3.3 P2-B: 名词访问器重命名

| 当前 | 修复后 |
|------|--------|
| `owner(def_id)` | `find_owner(def_id)` |
| `body(body_id)` | `find_body(body_id)` |
| `local_of(hir_id)` | `find_local(hir_id)` |
| `generics_of(def_id)` | `find_generics(def_id)` |

### 3.4 P2-C: pub fn → pub(crate)

降级非公共 API 的可见性:
- `hir/generics.rs`: `build_generics_map`, `generics_of`
- `hir/kinds.rs`: `hir_expr_kind_to_string`
- `traits/resolver.rs`: `summary`, `trait_count`, `impl_count`
- `mir/lower/field_resolution.rs`: `resolve_index_element_type`

### 3.5 P2-D: Span::DUMMY 清理 (HIGH 优先级)

14 处 HIGH 优先级 Span::DUMMY (错误报告):

| # | 文件 | 修复 |
|---|------|------|
| 1 | `parser/macro_expand.rs:3580` | 使用 `call_span` |
| 2-10 | `typeck/unify.rs` (9 处) | 添加 `span: Span` 参数 |
| 11-12 | `mir/lower/field_resolution.rs:271,283` | 使用 `expr.span` |
| 13 | `codegen/llvm/helpers.rs:62` | 添加 `span: Span` 参数 |
| 14 | `driver.rs` (missing main) | 使用 crate root span |

## 4. §6.3 委员会投票

| 角色 | 投票 | 备注 |
|------|------|------|
| ARCH-A | GO | API 标准化 + Span 清理 |
| REV-A | GO | 14 处 Span::DUMMY 是诊断质量关键 |
| DEV-A | GO | deprecated 别名保持兼容 |
| QA-A | GO | 全量回归验证 |
| PM-A | GO | 路线图项目 |

**5/5 GO** ✅
