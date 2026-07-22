# Stage 5.13 开发计划：trait impl 统计

> **阶段**: Stage 5.13
> **版本**: v0.11.11 → v0.11.12
> **状态**: ✅ Complete
> **流程**: stage-committee-process.md v3.20 §17.3 时期 1

## 1. 目标

添加 trait impl 统计查询方法，支持 diagnostics（"类型 S 实现了 N 个 trait"）
和 typeck trait-bound 求解。

## 2. 设计

### 2.1 新增 4 个查询方法

| 方法 | 签名 | 用途 |
|------|------|------|
| `impl_count_for_type` | `(def_id: DefId) -> usize` | 某类型有多少个 trait impl |
| `impl_count_for_trait` | `(trait_spur: Spur) -> usize` | 某 trait 有多少个实现 |
| `builtin_trait_count` | `() -> usize` | 内置 trait 数量 |
| `traits_for_type` | `(def_id: DefId) -> Vec<Spur>` | 某类型实现了哪些 trait |

### 2.2 命名标准化（API-naming-standard §3）

| 新增 API | 命名规则 | 备注 |
|----------|----------|------|
| `impl_count_for_type` | `impl_count_` 前缀 + `_for_type` 后缀 | 与 `impl_count` 一致 |
| `impl_count_for_trait` | `impl_count_` + `_for_trait` | 同上 |
| `builtin_trait_count` | `builtin_trait_` + `_count` | 与 `trait_count` 一致 |
| `traits_for_type` | `<noun>_for_<noun>` | 集合查询模式 |

## 3. 验收标准

1. `cargo fmt --check` 零 diff ✅
2. `cargo clippy --all-targets` 0 warnings ✅
3. `cargo test` 全部通过（954 → 961, +7 ✅）
4. §17.3 三阶段文档协议执行 ✅
5. §16 合规 ✅
6. API 命名遵循 api-naming-standard §3 ✅
7. §1.2 交付前验收：cargo clean+test+fmt+clippy 全绿 ✅

---

**创建日期**: 2026-07-22
