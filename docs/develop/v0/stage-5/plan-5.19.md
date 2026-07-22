# Stage 5.19 开发计划：trait impl completeness check

> **阶段**: Stage 5.19
> **版本**: v0.11.17 → v0.11.18
> **状态**: ✅ Complete
> **流程**: stage-committee-process.md v3.20 §17.3 时期 1

## 1. 目标

检测 incomplete impls — trait 声明了方法但 impl 没有提供。这在 Rust 中
是编译错误（"not all trait items implemented"）。

## 2. 设计

### 2.1 新增 3 个查询方法

| 方法 | 签名 | 用途 |
|------|------|------|
| `impl_covers_trait` | `(trait, ty) -> bool` | impl 是否覆盖所有 trait 方法 |
| `missing_impl_methods` | `(trait, ty) -> Vec<Spur>` | 缺失的方法名列表 |
| `missing_method_count` | `(trait, ty) -> usize` | 缺失方法数 |

### 2.2 命名标准化（API-naming-standard §3）

| 新增 API | 命名规则 | 备注 |
|----------|----------|------|
| `impl_covers_trait` | `<noun>_<verb>_<noun>` | 布尔查询 |
| `missing_impl_methods` | `<adj>_<noun>_<noun>` | 集合返回 |
| `missing_method_count` | `<noun>_count` | 与 `method_count_for_trait` 一致 |

## 3. 验收标准

1. `cargo fmt --check` 零 diff ✅
2. `cargo clippy --all-targets` 0 warnings ✅
3. `cargo test` 全部通过（999 → 1007, +8 ✅）— **1000+ 测试里程碑** 🎉
4. §17.3 三阶段文档协议执行 ✅
5. §16 合规 ✅
6. API 命名遵循 api-naming-standard §3 ✅
7. §1.2 交付前验收：cargo clean+test+fmt+clippy 全绿 ✅

---

**创建日期**: 2026-07-22
