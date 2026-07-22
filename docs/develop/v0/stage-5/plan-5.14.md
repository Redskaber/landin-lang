# Stage 5.14 开发计划：trait method query API

> **阶段**: Stage 5.14
> **版本**: v0.11.12 → v0.11.13
> **状态**: ✅ Complete
> **流程**: stage-committee-process.md v3.20 §17.3 时期 1

## 1. 目标

添加 trait method 查询 API，支持方法解析（method resolution）和 vtable
方法查找。

## 2. 设计

### 2.1 新增 5 个查询方法

| 方法 | 签名 | 用途 |
|------|------|------|
| `trait_methods` | `(trait_spur) -> Option<&Vec<Spur>>` | trait 声明的方法列表 |
| `impl_methods` | `(trait_spur, ty_spur) -> Option<&Vec<Spur>>` | impl 实现的方法列表 |
| `trait_has_method` | `(trait_spur, method_spur) -> bool` | trait 是否声明某方法 |
| `traits_with_method` | `(method_spur) -> Vec<Spur>` | 哪些 trait 声明了某方法 |
| `method_count_for_trait` | `(trait_spur) -> usize` | trait 的方法数 |

### 2.2 命名标准化（API-naming-standard §3）

| 新增 API | 命名规则 | 备注 |
|----------|----------|------|
| `trait_methods` | `<noun>_<noun>` | 集合查询 |
| `impl_methods` | `<noun>_<noun>` | 平行 trait_methods |
| `trait_has_method` | `<noun>_<verb>_<noun>` | 布尔查询 |
| `traits_with_method` | `<noun>_with_<noun>` | 集合返回 |
| `method_count_for_trait` | `<noun>_count_for_<noun>` | 与 impl_count_for_trait 一致 |

## 3. 验收标准

1. `cargo fmt --check` 零 diff ✅
2. `cargo clippy --all-targets` 0 warnings ✅
3. `cargo test` 全部通过（961 → 969, +8 ✅）
4. §17.3 三阶段文档协议执行 ✅
5. §16 合规 ✅
6. API 命名遵循 api-naming-standard §3 ✅
7. §1.2 交付前验收：cargo clean+test+fmt+clippy 全绿 ✅

---

**创建日期**: 2026-07-22
