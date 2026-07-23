# Stage 5.20 开发计划：trait impl validation report

> **阶段**: Stage 5.20
> **版本**: v0.11.18 → v0.11.19
> **状态**: ✅ Complete
> **流程**: stage-committee-process.md v3.20 §17.3 时期 1

## 1. 目标

将 coherence checking（5.18）和 completeness checking（5.19）聚合为
单一验证报告，提供 `validate_impls()` 单入口点。

## 2. 设计

### 2.1 新增 2 个结构体

| 结构体 | 字段 | 用途 |
|--------|------|------|
| `IncompleteImpl` | trait_name, self_ty_name, missing_methods | 单个不完整 impl |
| `ImplValidationReport` | coherence_errors, incomplete_impls, is_valid | 综合验证报告 |

### 2.2 新增 3 个查询方法

| 方法 | 签名 | 用途 |
|------|------|------|
| `validate_impls` | `() -> ImplValidationReport` | 单次验证所有 impl |
| `impls_are_valid` | `() -> bool` | 所有 impl 是否有效 |
| `all_impls_complete` | `() -> bool` | 所有 impl 是否完整 |

### 2.3 命名标准化（API-naming-standard §3）

| 新增 API | 命名规则 | 备注 |
|----------|----------|------|
| `IncompleteImpl` | `<Adj><Noun>` | 与 `CoherenceError` 一致 |
| `ImplValidationReport` | `<Noun>ValidationReport` | 报告类型 |
| `validate_impls` | `validate_<noun>` | 动作方法 |
| `impls_are_valid` | `<noun>_are_<adj>` | 布尔聚合 |
| `all_impls_complete` | `all_<noun>_<adj>` | 布尔聚合 |

## 3. 验收标准

1. `cargo fmt --check` 零 diff ✅
2. `cargo clippy --all-targets` 0 warnings ✅
3. `cargo test` 全部通过（1007 → 1016, +9 ✅）
4. §17.3 三阶段文档协议执行 ✅
5. §16 合规 ✅
6. API 命名遵循 api-naming-standard §3 ✅
7. §1.2 交付前验收：cargo clean+test+fmt+clippy 全绿 ✅

---

**创建日期**: 2026-07-22
