# Stage 5.17 开发计划：vtable method resolution

> **阶段**: Stage 5.17
> **版本**: v0.11.15 → v0.11.16
> **状态**: ✅ Complete
> **流程**: stage-committee-process.md v3.20 §17.3 时期 1

## 1. 目标

添加 vtable method resolution API，提供单一入口点将 `(trait, type, method)`
解析为具体 LLVM 符号名。结合 vtable 基础设施（5.5-5.6）与 trait 方法查询（5.14）。

## 2. 设计

### 2.1 新增 3 个查询方法

| 方法 | 签名 | 用途 |
|------|------|------|
| `resolve_vtable_method` | `(trait, ty, method) -> Option<&str>` | 解析方法符号名 |
| `vtable_method_names` | `(trait, ty) -> Vec<&str>` | 所有方法符号名 |
| `vtable_has_method` | `(trait, ty, method) -> bool` | vtable 是否有某方法 |

### 2.2 命名标准化（API-naming-standard §3）

| 新增 API | 命名规则 | 备注 |
|----------|----------|------|
| `resolve_vtable_method` | `resolve_<noun>_<noun>` | 解析查询 |
| `vtable_method_names` | `<noun>_<noun>_<noun>` | 集合返回 |
| `vtable_has_method` | `<noun>_<verb>_<noun>` | 布尔查询 |

## 3. 验收标准

1. `cargo fmt --check` 零 diff ✅
2. `cargo clippy --all-targets` 0 warnings ✅
3. `cargo test` 全部通过（984 → 992, +8 ✅）
4. §17.3 三阶段文档协议执行 ✅
5. §16 合规 ✅
6. API 命名遵循 api-naming-standard §3 ✅
7. §1.2 交付前验收：cargo clean+test+fmt+clippy 全绿 ✅

---

**创建日期**: 2026-07-22
