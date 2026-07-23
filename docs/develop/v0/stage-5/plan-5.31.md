# Stage 5.31 开发计划：stdlib facade

> **阶段**: Stage 5.31
> **版本**: v0.11.27 → v0.11.28
> **状态**: ✅ Complete

## 1. 目标

添加 `StdlibFacade` 结构体，提供 stdlib 聚合统计 + 层查询统一接口。

## 2. 设计

### 2.1 `StdlibFacade` 结构体

| 方法 | 签名 | 用途 |
|------|------|------|
| `from_prelude` | `(prelude) -> Self` | 从 prelude 构建 facade |
| `type_count` | `() -> usize` | 总类型数 |
| `trait_count` | `() -> usize` | 总 trait 数 |
| `type_count_for_layer` | `(layer) -> usize` | 某层类型数 |
| `layer_count` | `() -> usize` | 层数（3: Core/Alloc/Std） |
| `is_stdlib_name` | `(name) -> bool` | 是否 stdlib 提供 |
| `summary` | `() -> String` | 人类可读摘要 |

### 2.3 命名标准化

| API | 命名规则 |
|-----|---------|
| `StdlibFacade` | `<Noun><Noun>` |
| `from_prelude` | `from_<noun>` |
| `type_count` / `trait_count` | `<noun>_count` |
| `type_count_for_layer` | `<noun>_count_for_<noun>` |
| `is_stdlib_name` | `is_<noun>_<noun>` |
| `summary` | 名词（输出内容） |

## 3. 验收标准

1. `cargo fmt --check` 零 diff ✅
2. `cargo clippy --all-targets` 0 warnings ✅
3. `cargo test` 全部通过 ✅
4. §1.2 交付前验收：全绿 ✅

---

**创建日期**: 2026-07-23
