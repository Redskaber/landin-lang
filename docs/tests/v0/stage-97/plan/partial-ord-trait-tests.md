# Stage 97 测试计划 — PartialOrd trait (declared only)

## 测试目标

验证 `PartialOrd<Rhs>` trait 声明 (no impls) 不引入回归。Debug impls 移除以避免 crash, 等待 Stage 98 修复。

## 覆盖场景

| 场景 | 测试 | 类型 |
|------|------|------|
| trait 声明存在 | `stage97_partial_ord_trait_declared` | 正向 |
| undefined type 报错 | `stage97_undefined_type_errors` | 负向 |
| type mismatch 报错 | `stage97_type_mismatch_errors` | 负向 |
| nonexistent method 报错 | `stage97_nonexistent_method_errors` | 负向 |

## 对应代码

- 测试代码: `tests/v0/stage97/plan/partial_ord_trait_tests.rs`
- 实现代码: `src/stdlib/prelude.rs` (Stage 97 标记)

## 预期/实际

- 预期测试数: 4
- 实际测试数: 4 ✓

## 测试矩阵更新

| 维度 | 增量 |
|------|------|
| 总测试数 | 5576 → 5580 |
| 失败数 | 0 |
| ignored | 9 |
