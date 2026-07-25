# Stage 7.5 测试计划: Region inference integration with borrowck (TD-015 step 5)

> **阶段**: Stage 7.5
> **对应代码**: tests/v0/stage7/plan/region_inference_tests.rs
> **状态**: ✅ Complete

## 1. 测试目标

验证 TD-015 region inference 已集成到 borrow checker，且不影响现有 NLL 行为。
Region inference 作为额外的检查运行 (no-op when MIR regions all Erased → 'static)。

## 2. 覆盖场景

| 场景 | 测试函数名 | 状态 | 说明 |
|------|-----------|------|------|
| Context 创建 | test_region_inference_context_creation | ✅ | RegionInferenceContext::new() 成功 |
| 简单 body | test_region_inference_simple_body | ✅ | 无 ref 的 body 不报错 |
| Ref 类型 body | test_region_inference_ref_type_body | ✅ | 含 ref 的 body 走完 inference |
| 有效 borrow 接受 | test_region_inference_valid_borrow_accepted | ✅ | 合法 borrow 不报错 |
| Use-after-move 检测 | test_region_inference_use_after_move_detected | ✅ | 现有 NLL 仍能检测 use-after-move |
| Standalone context | test_region_inference_standalone_context | ✅ | RegionInferenceContext 可独立使用 |
| 回归: 空 body | test_regression_empty_body | ✅ | 空 MIR body 不 panic |
| 回归: Copy type 多次使用 | test_regression_copy_type_multi_use | ✅ | Copy type 多次使用仍合法 |

## 3. §17 测试矩阵对齐

| 矩阵项 | Stage 7.5 测试 |
|--------|----------------|
| 正面 (valid) | ✅ test_region_inference_valid_borrow_accepted |
| 负面 (soundness) | ✅ test_region_inference_use_after_move_detected |
| 边界 (empty) | ✅ test_regression_empty_body |
| 多态 (Copy type) | ✅ test_regression_copy_type_multi_use |
| 集成 | ✅ test_region_inference_ref_type_body |

## 4. 测试统计

- 预期: 8, 实际: 8 (2007 → 2015, +8 ✅)
- TD-015 step 5 final: borrowck integration complete

---

**创建日期**: 2026-07-25
