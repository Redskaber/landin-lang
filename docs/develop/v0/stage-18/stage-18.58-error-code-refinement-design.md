# Stage 18.58 — Error Code Catalog Refinement (ResolveErrorKind + TypeErrorKind)

> **Author**: redskaber + ARCH-A + DEV-A + QA-A
> **Date**: 2026-08-08
> **Version**: v0.324.0 → v0.325.0
> **Process**: stage-committee-process.md v5.0 §13.1 + §13.5 + §14 (deep review)
> **Status**: ✅ Design Complete — Ready for Implementation

---

## 1. 背景 (§13.1 阶段开始设计对齐)

### 1.1 审计发现 (Stage 18.56)

错误系统精度审计发现:
- `BorrowError` 有结构化 `BorrowErrorKind` enum (9 kinds) ✓
- `ResolveError` 无 kind enum — 仅自由格式 `message: String` ✗
- `TypeError` 无 kind enum — 仅自由格式 `message: String` + expected/found ✗
- 错误码目录太粗: 8 codes (E001/E100/E300/E400/E500/E600/E900) vs rustc 600+

### 1.2 本阶段目标

为 `ResolveError` 和 `TypeError` 添加 kind enum, 镜像 `BorrowErrorKind` 设计, 实现机器可读的错误分类。

**做**:
- 新增 `ResolveErrorKind` enum (CannotFindType, CannotFindValue, CannotFindTrait, CannotFindMacro, DuplicateDefinition, AssocTypeNotFound, etc.)
- 新增 `TypeErrorKind` enum (MismatchedTypes, UnresolvedType, etc.)
- `ResolveError` / `TypeError` 增加 `kind` 字段
- 向后兼容: 保留 `new(message, span)` 构造器 (kind = Generic)
- 新增 `with_kind(kind, message, span)` 构造器
- 迁移主要调用点使用 kind enum

**不做** (留待后续):
- ❌ 细化 `ErrorCode` 目录到 per-error-pattern (需迁移所有诊断)
- ❌ `MacroError` / `TraitError` kind enum (后续 stage)

### 1.3 设计原则遵循

| 原则 | 如何遵循 |
|------|---------|
| 3. 显式 > 隐式 | kind enum 显式分类, 不依赖字符串匹配 |
| 4. 报错 > 静默 | 错误分类清晰, 可机器处理 |
| 6. 通用 > 特例 | 一个 kind enum 处理所有错误模式 |
| 7. API 命名标准化 | `with_kind` / `ResolveErrorKind` 命名 |

---

## 2. 技术设计

### 2.1 ResolveErrorKind (src/resolve/error.rs)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolveErrorKind {
    /// Generic resolve error (backward compat for `new(message, span)`).
    Generic,
    /// `cannot find type in this scope` — undefined type name.
    CannotFindType,
    /// `cannot find value in this scope` — undefined value/function.
    CannotFindValue,
    /// `cannot find trait in this scope` — undefined trait.
    CannotFindTrait,
    /// `cannot find macro in this scope` — undefined macro.
    CannotFindMacro,
    /// `duplicate definition for X` — name collision.
    DuplicateDefinition,
    /// `associated type X not found in trait` — invalid assoc type reference.
    AssocTypeNotFound,
    /// `cannot find trait in qualified path` — undefined trait in `<T as Trait>::Item`.
    UndefinedTraitInQualified,
}
```

### 2.2 TypeErrorKind (src/typeck/error.rs)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeErrorKind {
    /// Generic type error (backward compat for `new(message, span)`).
    Generic,
    /// `mismatched types: expected X, found Y`.
    MismatchedTypes,
    /// Type contains unresolved inference variable.
    UnresolvedType,
    /// `cannot find type` propagated from resolve.
    UnresolvedName,
    /// Trait bound not satisfied.
    TraitBoundNotSatisfied,
    /// Function signature mismatch (arity, types).
    SignatureMismatch,
}
```

### 2.3 向后兼容策略

- 保留 `ResolveError::new(message, span)` → kind = `Generic`
- 保留 `TypeError::new(message, span)` → kind = `Generic`
- 保留 `TypeError::mismatch(expected, found, span)` → kind = `MismatchedTypes`
- 新增 `ResolveError::with_kind(kind, message, span)`
- 新增 `TypeError::with_kind(kind, message, span)`
- 迁移调用点逐步使用 `with_kind`, 不破坏现有代码

---

## 3. §6.3 委员会投票 (模拟)

| 角色 | 投票 | 备注 |
|------|------|------|
| ARCH-A | GO | 镜像 BorrowErrorKind 设计, 风险低 |
| DEV-A | GO | 向后兼容, 渐进迁移 |
| QA-A | GO | 可测试 error kind 分类 |
| REV-A | GO | 审计驱动, 错误系统精度提升 |
| PM-A | GO | 技术债清理 |

**5/5 GO** ✅
