# Stage 5.18 开发计划：trait coherence checking

> **阶段**: Stage 5.18
> **版本**: v0.11.16 → v0.11.17
> **状态**: ✅ Complete
> **流程**: stage-committee-process.md v3.20 §17.3 时期 1

## 1. 目标

检测 conflicting impls（同一 `(trait, type)` pair 有多个 impl），
这是 Rust 中的 hard error（"conflicting implementations"）。

## 2. 设计

### 2.1 `CoherenceError` 结构体

```rust
pub struct CoherenceError {
    pub trait_name: Spur,
    pub self_ty_name: Spur,
    pub impl_def_ids: Vec<DefId>,
}
```

### 2.2 新增 3 个查询方法

| 方法 | 签名 | 用途 |
|------|------|------|
| `check_coherence` | `() -> Vec<CoherenceError>` | 检测所有冲突 |
| `has_coherence_error` | `(trait, ty) -> bool` | 某对是否有冲突 |
| `coherence_error_count` | `() -> usize` | 冲突数量 |

### 2.3 命名标准化（API-naming-standard §3）

| 新增 API | 命名规则 | 备注 |
|----------|----------|------|
| `CoherenceError` | `<Noun>Error` | 与 `TypeError`/`BorrowError` 一致 |
| `check_coherence` | `check_<noun>` | 与 `check_visibility` 一致 |
| `has_coherence_error` | `has_<noun>` | 布尔查询 |
| `coherence_error_count` | `<noun>_count` | 与 `trait_count`/`impl_count` 一致 |

## 3. 验收标准

1. `cargo fmt --check` 零 diff ✅
2. `cargo clippy --all-targets` 0 warnings ✅
3. `cargo test` 全部通过（992 → 999, +7 ✅）
4. §17.3 三阶段文档协议执行 ✅
5. §16 合规 ✅
6. API 命名遵循 api-naming-standard §3 ✅
7. §1.2 交付前验收：cargo clean+test+fmt+clippy 全绿 ✅

---

**创建日期**: 2026-07-22
