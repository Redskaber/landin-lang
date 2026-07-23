# Stage 5.29 开发计划：stdlib layer query

> **阶段**: Stage 5.29
> **版本**: v0.11.25 → v0.11.26
> **状态**: ✅ Complete
> **流程**: stage-committee-process.md v3.20 §17.3 时期 1

## 1. 目标

添加 `StdlibLayer` 枚举 + `layer_for_name()` + `names_for_layer()` 查询方法，
支持按 stdlib 层（core/alloc/none）查询类型归属。

## 2. 设计

### 2.1 新增 `StdlibLayer` 枚举

```rust
pub enum StdlibLayer { Core, Alloc, None }
```

### 2.2 新增查询方法

| 方法 | 签名 | 用途 |
|------|------|------|
| `layer_for_name` | `(&self, name: &str) -> StdlibLayer` | 查询名称所属层 |
| `names_for_layer` | `(&self, layer: StdlibLayer) -> Vec<&'static str>` | 获取某层所有名称 |

### 2.3 命名标准化

| API | 命名规则 |
|-----|---------|
| `StdlibLayer` | `<Noun><Noun>` |
| `layer_for_name` | `<noun>_for_<noun>` |
| `names_for_layer` | `<noun>_for_<noun>` |

## 3. 验收标准

1. `cargo fmt --check` 零 diff ✅
2. `cargo clippy --all-targets` 0 warnings ✅
3. `cargo test` 全部通过 ✅
4. §1.2 交付前验收：全绿 ✅
5. 补充所有缺失的 docs/tests/v0/stage5/{gate,plan} 文档 ✅

---

**创建日期**: 2026-07-23
