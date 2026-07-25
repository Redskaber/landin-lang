# Stage 8.1 测试计划: Lifetime elision rules (§3.2 RFC #141)

> **阶段**: Stage 8.1
> **对应代码**: tests/v0/stage8/plan/lifetime_elision_tests.rs
> **状态**: ✅ Complete

## 1. 测试目标

验证 §3.2 lifetime elision rules (Rust RFC #141) — 在没有显式 lifetime 标注的
情况下，编译器能自动推断 lifetime 关系。验证 `src/typeck/lifetime_elision.rs`
模块存在并集成到 driver pipeline。

## 2. 覆盖场景

| 场景 | 测试函数名 | 状态 | 说明 |
|------|-----------|------|------|
| 模块存在 | test_lifetime_elision_module_exists | ✅ | src/typeck/lifetime_elision.rs 存在 |
| Pipeline with refs | test_pipeline_with_refs_no_panic | ✅ | 含 &i32 的 fn 不 panic |
| Simple fn | test_simple_fn_no_refs | ✅ | 无 ref 的 fn 正常 |
| Ref param | test_ref_param_elided | ✅ | `fn f(x: &i32)` elision rule 1 |
| Mut ref | test_mut_ref_param_elided | ✅ | `fn f(x: &mut i32)` elision rule 1 |
| Nested refs | test_nested_refs_elided | ✅ | `fn f(x: &&i32)` elision 递归 |
| Ref return | test_ref_return_elided | ✅ | `fn f(x: &i32) -> &i32` elision rule 2 |

## 3. §17 测试矩阵对齐

| 矩阵项 | Stage 8.1 测试 |
|--------|----------------|
| 正面 (rule 1) | ✅ test_ref_param_elided + test_mut_ref_param_elided |
| 正面 (rule 2) | ✅ test_ref_return_elided |
| 边界 (无 ref) | ✅ test_simple_fn_no_refs |
| 多态 (nested) | ✅ test_nested_refs_elided |
| 集成 | ✅ test_pipeline_with_refs_no_panic |

## 4. 测试统计

- 预期: 7, 实际: 7 (2042 → 2052, +7 ✅)
- 另有 3 unit tests inline in lifetime_elision.rs

---

**创建日期**: 2026-07-25
