# Stage 96 测试计划 — Ord trait (marker) coverage

## 测试目标

验证 `Ord` trait (marker only, no method body) 不引入回归, 并触发新 TD 记录。

## 覆盖场景

| 场景 | 测试 | 类型 |
|------|------|------|
| trait 声明存在 | `stage96_ord_trait_declared` | 正向 |
| undefined type 报错 | `stage96_undefined_type_errors` | 负向 |
| type mismatch 报错 | `stage96_type_mismatch_errors` | 负向 |
| nonexistent method 报错 | `stage96_nonexistent_method_errors` | 负向 |

## 对应代码

- 测试代码: `tests/v0/stage96/plan/ord_trait_tests.rs`
- 实现代码: `src/stdlib/prelude.rs` (Stage 96 标记)

## 预期/实际

- 预期测试数: 4
- 实际测试数: 4 ✓

## 测试矩阵更新

| 维度 | 增量 |
|------|------|
| 总测试数 | 5572 → 5576 |
| 失败数 | 0 |
| ignored | 9 |
