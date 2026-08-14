# Stage 18.81 — P2 API Refactoring (unify span + get_ prefix + noun accessors)

> **Author**: redskaber
> **Date**: 2026-08-09
> **Version**: v0.348.0 → v0.349.0
> **Process**: stage-committee-process.md v5.0 §13.1 (设计对齐) + §13.5 (设计-审查) + §14 (深度审查)
> **Status**: ✅ Complete

## 1. 背景

Stage 18.80 延后了 4 项 P2 API 重构。本 Stage 处理这些延后项。

## 2. 修复项

| # | 描述 | 修复方案 |
|---|------|---------|
| P2-1 | unify() 9 处 Span::DUMMY | 为 unify/unify_resolved 添加 span 参数, 更新 32 个调用点 |
| P2-2 | 11 处 get_ 前缀 | 重命名 + deprecated 别名 |
| P2-3 | 6 处名词访问器 | 重命名 + deprecated 别名 |

### 2.1 P2-1: unify span 参数

**最高优先级** — 修复 9 处 HIGH 优先级 Span::DUMMY (typeck/unify.rs)

```rust
// Before:
pub fn unify(&mut self, a: &Ty, b: &Ty) -> Result<(), Box<TypeError>>
fn unify_resolved(&mut self, a: &Ty, b: &Ty) -> Result<(), Box<TypeError>>

// After:
pub fn unify(&mut self, a: &Ty, b: &Ty, span: Span) -> Result<(), Box<TypeError>>
fn unify_resolved(&mut self, a: &Ty, b: &Ty, span: Span) -> Result<(), Box<TypeError>>
```

所有 `make_mismatch` 调用使用传入的 `span` 而非 `Span::DUMMY`。

### 2.2 P2-2: get_ 前缀移除

| 当前 | 新名 | deprecated 别名 |
|------|------|----------------|
| `get_local` | `local` | `get_local` (deprecated) |
| `get_local_ptr` | `local_ptr` | `get_local_ptr` (deprecated) |
| `get_struct_fields` | `struct_fields` | — |
| `get_const_val` | `const_val` | — |
| `get_const_ty` | `const_ty` | — |
| `get_or_declare_function` | `declare_function` | — |
| `get_call_dest_type` | `call_dest_type` | — |

### 2.3 P2-3: 名词访问器重命名

| 当前 | 新名 |
|------|------|
| `owner(def_id)` | `find_owner(def_id)` |
| `body(body_id)` | `find_body(body_id)` |
| `local_of(hir_id)` | `find_local(hir_id)` |
| `generics_of(def_id)` | `find_generics(def_id)` |

## 3. §6.3 委员会投票

| 角色 | 投票 | 备注 |
|------|------|------|
| ARCH-A | GO | API 标准化 + 诊断质量 |
| REV-A | GO | 9 处 unify Span::DUMMY 是关键 |
| DEV-A | GO | deprecated 别名保持兼容 |
| QA-A | GO | 全量回归验证 |
| PM-A | GO | 路线图项目 |

**5/5 GO** ✅
