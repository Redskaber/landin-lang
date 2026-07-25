# Stage 8.2 测试计划: Object safety rules (§2.3 RFC #255)

> **阶段**: Stage 8.2
> **对应代码**: tests/v0/stage8/plan/object_safety_tests.rs
> **状态**: ✅ Complete

## 1. 测试目标

验证 §2.3 object safety rules (Rust RFC #255) — `dyn Trait` 仅能用于 object-safe
的 trait。验证 `src/traits/object_safety.rs` 模块正确实现 4 条规则。

## 2. 覆盖场景

| 场景 | 测试函数名 | 状态 | 说明 |
|------|-----------|------|------|
| 模块存在 | test_object_safety_module_exists | ✅ | src/traits/object_safety.rs 存在 |
| Object-safe trait | test_object_safe_trait | ✅ | `trait T { fn f(&self); }` → safe |
| Self return 不 safe | test_returns_self_not_safe | ✅ | `fn f(&self) -> Self` → not safe |
| Generic method 不 safe | test_generic_method_not_safe | ✅ | `fn f<T>(&self, x: T)` → not safe |
| &self receiver safe | test_ref_self_receiver_safe | ✅ | `fn f(&self)` → safe |

## 3. §17 测试矩阵对齐

| 矩阵项 | Stage 8.2 测试 |
|--------|----------------|
| 正面 (safe) | ✅ test_object_safe_trait + test_ref_self_receiver_safe |
| 负面 (not safe) | ✅ test_returns_self_not_safe + test_generic_method_not_safe |
| 集成 | ✅ test_object_safety_module_exists |

## 4. 测试统计

- 预期: 5, 实际: 5 (2052 → 2062, +5 ✅)
- 另有 5 unit tests inline in object_safety.rs

---

**创建日期**: 2026-07-25
