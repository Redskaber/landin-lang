# Stage 5.12 开发计划：Copy 检测统一化

> **阶段**: Stage 5.12
> **版本**: v0.11.10 → v0.11.11
> **状态**: ✅ Complete
> **流程**: stage-committee-process.md v3.20 §17.3 时期 1

## 1. 目标

将 Stage 5.11 的 `is_primitive_copy_kind()` 接入 `ty_is_copy_with_resolver`，
实现 Copy 检测的单一信息源。新增 `ty_is_copy_unified()` 作为新代码的首选入口。

## 2. 背景

Stage 5.11 添加了 `is_primitive_copy_kind()`，但 `ty_is_copy_with_resolver`
仍硬编码 primitive 分支（`Bool | Char | Int(_) | ... => true`）。Stage 5.12
将 primitive 分支委托给 `is_primitive_copy_kind()`，确保 Copy 知识的单一来源。

## 3. 设计

### 3.1 `ty_is_copy_with_resolver` 重构

primitive 分支从硬编码 `=> true` 改为：
```rust
Bool | Char | Int(_) | ... => is_primitive_copy_kind(&format!("{:?}", ty.kind))
```

match 仍处理 Tuple/Array（递归）和 Adt（resolver 查询）——这些
`is_primitive_copy_kind` 无法处理（它是字符串检查，无递归）。

### 3.2 `ty_is_copy_unified()` 新入口

```rust
pub fn ty_is_copy_unified(ty, resolver, interner) -> bool
```

委托给 `ty_is_copy_with_resolver`。独立命名以表达"统一"意图，是新代码
的首选入口。

### 3.3 命名标准化（API-naming-standard §3）

| 新增 API | 命名规则 | 备注 |
|----------|----------|------|
| `ty_is_copy_unified` | `ty_is_copy_` 前缀 + `_unified` 后缀 | 与 `ty_is_copy` / `ty_is_copy_with_resolver` 一致 |

## 4. 验收标准

1. `cargo fmt --check` 零 diff ✅
2. `cargo clippy --all-targets` 0 warnings ✅
3. `cargo test` 全部通过（949 → 954, +5 ✅）
4. §17.3 三阶段文档协议执行 ✅
5. §16 合规 ✅
6. API 命名遵循 api-naming-standard §3 ✅
7. §1.2 交付前验收：cargo clean+test+fmt+clippy 全绿 ✅

---

**创建日期**: 2026-07-22
