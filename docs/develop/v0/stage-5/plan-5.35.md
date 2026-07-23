# Stage 5.35 开发计划：stdlib type layout

> **阶段**: Stage 5.35
> **版本**: v0.11.30 → v0.11.31
> **状态**: ✅ Complete

## 1. 目标

添加 `type_size_bytes()` + `type_alignment_bytes()` + `is_zero_sized_type()` +
`type_description()` 用于查询原始类型的 size/alignment/ZST/description。

## 2. 设计

### 2.1 新增 API

| API | 签名 | 用途 |
|-----|------|------|
| `type_size_bytes` | `(name: &str) -> Option<u64>` | 类型大小（字节） |
| `type_alignment_bytes` | `(name: &str) -> Option<u64>` | 类型对齐（字节） |
| `is_zero_sized_type` | `(name: &str) -> bool` | 是否 ZST |
| `type_description` | `(name: &str) -> Option<&'static str>` | 人类可读描述 |

### 2.3 命名标准化

| API | 命名规则 |
|-----|---------|
| `type_size_bytes` | `<noun>_<noun>_<noun>` |
| `type_alignment_bytes` | `<noun>_<noun>_<noun>` |
| `is_zero_sized_type` | `is_<adj>_<noun>` |
| `type_description` | `<noun>_<noun>` |

## 3. 验收标准

1. `cargo fmt --check` 零 diff ✅
2. `cargo clippy --all-targets` 0 warnings ✅
3. `cargo test` 全部通过 ✅
4. §1.2 交付前验收：全绿 ✅

---

**创建日期**: 2026-07-23
