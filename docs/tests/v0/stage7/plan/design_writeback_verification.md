# Stage 7.7 测试计划: §25.8 design writeback verification

> **阶段**: Stage 7.7
> **对应代码**: tests/v0/stage7/plan/design_writeback_verification_tests.rs
> **状态**: ✅ Complete

## 1. 测试目标

验证 §25.8 design writeback — Stage 7 的 TD-015 + TD-018 实现状态已同步到
设计文档 (03-type-system.md + 04-ownership-borrowing.md)。通过运行实际编译
流程验证设计文档中描述的行为与实现一致。

## 2. 覆盖场景

| 场景 | 测试函数名 | 状态 | 说明 |
|------|-----------|------|------|
| TD-015 borrow checker 运行 region inference | test_td015_borrow_checker_runs_region_inference | ✅ | check_mir_body 调用 run_region_inference |
| TD-015 处理 ref 类型 | test_td015_handles_ref_types | ✅ | 含 &i32 的 body 不报错 |
| TD-015 处理嵌套 ref | test_td015_handles_nested_refs | ✅ | 含 &&i32 的 body 不报错 |
| TD-018 resolver-based method calls 存在 | test_td018_resolver_based_method_calls_exist | ✅ | build_dyn_trait_method_calls_from_resolver 函数存在 |
| TD-018 user-defined trait 解析 | test_td018_user_defined_trait_resolved | ✅ | TraitResolver.vtables 含 user-defined trait |
| TD-018 stdlib + user 共存 | test_td018_stdlib_and_user_coexist | ✅ | stdlib trait + user-defined trait 各自处理 |

## 3. §17 测试矩阵对齐

| 矩阵项 | Stage 7.7 测试 |
|--------|----------------|
| TD-015 正面 | ✅ test_td015_borrow_checker_runs_region_inference |
| TD-015 多态 | ✅ test_td015_handles_nested_refs |
| TD-018 正面 | ✅ test_td018_resolver_based_method_calls_exist |
| TD-018 共存 | ✅ test_td018_stdlib_and_user_coexist |

## 4. 测试统计

- 预期: 6, 实际: 6 (2023 → 2029, +6 ✅)
- §25.8 design writeback verified for 2 design docs

---

**创建日期**: 2026-07-25
