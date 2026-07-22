# Stage 5.9 开发计划：builtin Copy 激活 + 健全性修复

> **阶段**: Stage 5.9
> **版本**: v0.11.7 → v0.11.8
> **状态**: ✅ Complete
> **流程**: stage-committee-process.md v3.19 §17.3 时期 1

## 1. 目标

激活 Stage 5.8 的 builtin trait 注册表，使 `impl Copy for S` 无需
`trait Copy {}` 即可工作。同时修复 `ty_is_copy_with_resolver` 的不健全
fallback（旧代码在 "Copy" 未 interned 时返回 `true`，将所有 Adt 视为
Copy——这是 soundness bug）。

## 2. 背景

Stage 5.8 添加了 `register_builtin_traits()`，使 "Copy" 总是被 interned。
但 `ty_is_copy_with_resolver` 的 Adt 分支仍使用旧的 fallback 逻辑：
```rust
if let Some(copy_name) = interner.get("Copy") {
    resolver.is_copy(*def_id, copy_name)
} else {
    true  // 不健全！将所有 Adt 视为 Copy
}
```

Stage 5.8 之后，`interner.get("Copy")` 总是返回 `Some`，所以 else 分支
不再执行。但代码仍不清晰——应该显式使用 builtin Copy 检测。

## 3. 设计

### 3.1 `is_copy_builtin()` 方法

新增 `pub fn is_copy_builtin(&self, def_id: DefId, interner: &Rodeo) -> bool`：
- 不需要调用者传入 Copy Spur（自动从 interner 查找）
- 如果 "Copy" 未 interned（防御性），返回 `false`（而非旧的 `true`）
- 这是下游阶段（borrowck/typeck）的首选 Copy 检测入口

### 3.2 `ty_is_copy_with_resolver` Adt 分支简化

```rust
// 旧代码（不健全 fallback）:
Adt(def_id, _) => {
    if let Some(copy_name) = interner.get("Copy") {
        resolver.is_copy(*def_id, copy_name)
    } else {
        true  // 不健全
    }
}

// 新代码（Stage 5.9，健全）:
Adt(def_id, _) => resolver.is_copy_builtin(*def_id, interner),
```

### 3.3 命名标准化（API-naming-standard §3）

| 新增 API | 命名规则 | 备注 |
|----------|----------|------|
| `TraitResolver::is_copy_builtin` | `is_` 前缀 + `_builtin` 后缀 | 与 `is_copy` 一致，`_builtin` 表明使用内置 trait |

## 4. MUV 拆分

| 子任务 | 描述 | 复杂度 |
|--------|------|--------|
| 5.9-a | `is_copy_builtin()` 方法 | L1 |
| 5.9-b | `ty_is_copy_with_resolver` Adt 分支简化 + 健全性修复 | L1 |
| 5.9-c | 更新 `test_adt_fallback_copy` 测试（改为 `test_adt_without_copy_impl_not_copy`） | L1 |
| 5.9-d | 新增 `builtin_copy_activation_tests.rs` (5 用例) | L1 |
| 5.9-e | lib.rs 文档注释更新 | L1 |
| 5.9-f | Cargo.toml 版本 + all_tests.rs 模块注册 | L1 |

## 5. 验收标准

1. `cargo fmt --check` 零 diff ✅
2. `cargo clippy --all-targets` 0 warnings ✅
3. `cargo test` 全部通过（931 → 936, +5 ✅）
4. §17.3 三阶段文档协议执行 ✅
5. §16 合规 ✅
6. API 命名遵循 api-naming-standard §3 ✅
7. Soundness: Adt without `impl Copy` → NOT Copy（不再 fallback true）✅

---

**创建日期**: 2026-07-22
