# Stage 5.26 开发计划：driver stdlib integration

> **阶段**: Stage 5.26
> **版本**: v0.11.23 → v0.11.24
> **状态**: ✅ Complete
> **流程**: stage-committee-process.md v3.20 §17.3 时期 1

## 1. 目标

将 `register_stdlib()`（Stage 5.25）接入 driver，使所有 stdlib 类型 +
trait 在编译时自动 interned。添加 `CompileResult.stdlib_prelude` 字段。

## 2. 设计

### 2.1 `CompileResult.stdlib_prelude` 字段

新增 `stdlib_prelude: StdlibPrelude` 字段，存储编译时使用的 prelude。

### 2.2 driver `register_stdlib()` 调用

在 `register_builtin_traits()` 后、`collect()` 前调用
`register_stdlib(&mut interner)`，确保所有 stdlib 名已 interned。

### 2.3 `empty()` 路径

lex/parse 错误路径也通过 `empty()` 提供 `default_prelude()`。

## 3. 验收标准

1. `cargo fmt --check` 零 diff ✅
2. `cargo clippy --all-targets` 0 warnings ✅
3. `cargo test` 全部通过（1041 → 1049, +8 ✅）
4. §1.2 交付前验收：全绿 ✅

---

**创建日期**: 2026-07-23
