# Stage 5.25 开发计划：stdlib MVP

> **阶段**: Stage 5.25
> **版本**: v0.11.22 → v0.11.23
> **状态**: ✅ Complete
> **流程**: stage-committee-process.md v3.20 §17.3 时期 1

## 1. 目标

实现 `core` 层 stdlib MVP——核心类型定义 + ops/convert/iter trait 名注册 +
prelude 管理。

## 2. 设计

### 2.1 新增 `src/stdlib.rs` 模块

| 常量/类型/函数 | 用途 |
|---------------|------|
| `STDLIB_CORE_TYPES` | 17 个核心类型（i8-i128/u8-u128/f32/f64/bool/char/str/()/Never） |
| `STDLIB_OPS_TRAITS` | 运算符 trait（Add/Sub/Mul/.../PartialEq/Ord/...） |
| `STDLIB_CONVERT_TRAITS` | 转换 trait（From/Into/TryFrom/AsRef/...） |
| `STDLIB_ITER_TRAITS` | 迭代器 trait（Iterator/IntoIterator/...） |
| `all_stdlib_trait_names()` | 去重排序的 trait 名列表 |
| `all_stdlib_type_names()` | 核心类型名列表 |
| `StdlibPrelude` | prelude 类型（types + traits） |
| `register_stdlib(&mut Rodeo)` | 在 interner 中注册所有 stdlib 名 |
| `default_prelude()` | 获取默认 prelude |

### 2.2 §16 合规

`register_stdlib()` 在 driver 阶段调用，使用 `&mut Rodeo`。无 HIR 访问。

### 2.3 命名标准化

| API | 命名规则 |
|-----|---------|
| `STDLIB_CORE_TYPES` | `SCREAMING_SNAKE_CASE` |
| `StdlibPrelude` | `<Noun><Noun>` |
| `register_stdlib` | `register_<noun>` |
| `default_prelude` | `<adj>_<noun>` |

## 3. 验收标准

1. `cargo fmt --check` 零 diff ✅
2. `cargo clippy --all-targets` 0 warnings ✅
3. `cargo test` 全部通过（1031 → 1041, +10 ✅）
4. §1.2 交付前验收：全绿 ✅

---

**创建日期**: 2026-07-23
