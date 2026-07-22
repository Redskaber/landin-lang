# Stage 5.15 开发计划：trait hierarchy（supertraits）

> **阶段**: Stage 5.15
> **版本**: v0.11.13 → v0.11.14
> **状态**: ✅ Complete
> **流程**: stage-committee-process.md v3.20 §17.3 时期 1

## 1. 目标

收集 trait 的 supertrait 信息（`trait Foo: Bar` 中的 `Bar`），支持
trait hierarchy 遍历和 typeck trait-bound 求解。

## 2. 设计

### 2.1 `TraitInfo.supertraits` 字段

新增 `supertraits: Vec<Spur>` 字段，在 `collect()` 时从
`HirTrait.supertraits`（`Vec<HirTypeBound>`）提取每个 bound 的 trait
path 最后一段 name Spur。

### 2.2 新增 3 个查询方法

| 方法 | 签名 | 用途 |
|------|------|------|
| `trait_supertraits` | `(trait_spur) -> Option<&Vec<Spur>>` | supertrait 列表 |
| `trait_has_supertrait` | `(trait_spur, super_spur) -> bool` | 是否有某 supertrait |
| `supertrait_count_for_trait` | `(trait_spur) -> usize` | supertrait 数量 |

### 2.3 命名标准化（API-naming-standard §3）

| 新增 API | 命名规则 | 备注 |
|----------|----------|------|
| `trait_supertraits` | `<noun>_<noun>` | 与 `trait_methods` 一致 |
| `trait_has_supertrait` | `<noun>_<verb>_<noun>` | 与 `trait_has_method` 一致 |
| `supertrait_count_for_trait` | `<noun>_count_for_<noun>` | 与 `method_count_for_trait` 一致 |

## 3. 验收标准

1. `cargo fmt --check` 零 diff ✅
2. `cargo clippy --all-targets` 0 warnings ✅
3. `cargo test` 全部通过（969 → 977, +8 ✅）
4. §17.3 三阶段文档协议执行 ✅
5. §16 合规 ✅
6. API 命名遵循 api-naming-standard §3 ✅
7. §1.2 交付前验收：cargo clean+test+fmt+clippy 全绿 ✅

---

**创建日期**: 2026-07-22
