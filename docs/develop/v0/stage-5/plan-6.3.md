# Stage 6.3 开发计划：mir/lower/mod.rs 拆分 — pattern bindings 提取（TD-011 第三步）

> **阶段**: Stage 6.3
> **版本**: v0.12.1 → v0.12.2
> **状态**: ✅ Complete

## 1. 目标

继续偿还 TD-011。第三步：将 pattern binding 相关函数从
`mir/lower/mod.rs` 提取到独立模块 `mir/lower/pattern_bindings.rs`。

## 2. 提取的函数

| 函数 | 行号 | 职责 |
|------|------|------|
| `pat_mutability` | 2391 | 检查 pattern 是否为 mut |
| `collect_pat_bindings_for_mir` | 2757 | 收集 pattern bindings 到 MIR locals |
| `lower_enum_variant_pattern_bindings` | 2808 | 降低 enum variant pattern bindings |
| `compute_enum_payload_starting_idx` | 2953 | 计算 enum variant payload 起始索引 |
| `collect_pat_hir_ids` | 2990 | 收集 pattern 中的所有 HirId |

---

**创建日期**: 2026-07-24
