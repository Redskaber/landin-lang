# Stage 5.11 开发计划：primitive Copy 自动检测

> **阶段**: Stage 5.11
> **版本**: v0.11.9 → v0.11.10
> **状态**: ✅ Complete
> **流程**: stage-committee-process.md v3.20 §17.3 时期 1

## 1. 目标

添加 `BUILTIN_PRIMITIVE_COPY_KINDS` 常量 + `is_primitive_copy_kind()` 函数，
使编译器能识别哪些 MIR `TyKind` 是 always-Copy（Bool/Char/Int/Uint/Float/
Never/Ref/RawPtr/FnDef/FnPtr），无需 trait resolver。这是 stdlib MVP 的
primitive auto-Copy 基础。

## 2. 背景

Stage 5.9 修复了 Adt 的 Copy 检测（无 `impl Copy` → false）。但 primitive
类型（i32, bool 等）的 Copy 检测仍硬编码在 `ty_is_copy_with_resolver` 的
match 分支中。Stage 5.11 将这个知识提取为可查询的常量 + 函数，供其他
阶段（typeck, codegen）复用。

## 3. 设计

### 3.1 `BUILTIN_PRIMITIVE_COPY_KINDS` 常量

```rust
pub const BUILTIN_PRIMITIVE_COPY_KINDS: &[&str] = &[
    "Bool", "Char", "Int", "Uint", "Float", "Never",
    "Ref", "RawPtr", "FnDef", "FnPtr",
];
```

10 个 always-Copy TyKind 变体名（字符串形式，避免 `traits`↔`mir` 循环依赖）。

### 3.2 `is_primitive_copy_kind()` 函数

```rust
pub fn is_primitive_copy_kind(kind_name: &str) -> bool
```

接受 `TyKind` 的 `Debug` 输出（如 `"Int(I32)"`），剥离 `(…) ` 后查表。

### 3.3 命名标准化（API-naming-standard §3）

| 新增 API | 命名规则 | 备注 |
|----------|----------|------|
| `BUILTIN_PRIMITIVE_COPY_KINDS` | SCREAMING_SNAKE_CASE 常量 | 与 `BUILTIN_TRAIT_NAMES` 一致 |
| `is_primitive_copy_kind` | `is_` 前缀 + `_kind` 后缀 | `_kind` 区别于 DefId-based `is_copy_builtin` |

## 4. 验收标准

1. `cargo fmt --check` 零 diff ✅
2. `cargo clippy --all-targets` 0 warnings ✅
3. `cargo test` 全部通过（943 → 949, +6 ✅）
4. §17.3 三阶段文档协议执行 ✅
5. §16 合规 ✅
6. API 命名遵循 api-naming-standard §3 ✅
7. §1.2 交付前验收：cargo clean+test+fmt+clippy 全绿 ✅

---

**创建日期**: 2026-07-22
