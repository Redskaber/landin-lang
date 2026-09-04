# Stage 94 测试计划 — Default trait coverage

## 测试目标

验证 `Default` trait 声明 + 4 个 primitive impls (i32/i64/bool/usize) 不引入回归，且 prelude trait 系统可正确编译/解析。

## 覆盖场景

| 场景 | 测试 | 类型 |
|------|------|------|
| trait 声明存在 | `stage94_default_trait_declared` | 正向 |
| undefined type 报错 | `stage94_undefined_type_errors` | 负向 (resolve) |
| type mismatch 报错 | `stage94_type_mismatch_errors` | 负向 (typeck) |
| nonexistent method 报错 | `stage94_nonexistent_method_errors` | 负向 (typeck/lower) |

## 对应代码

- 测试代码: `tests/v0/stage94/plan/default_trait_tests.rs`
- 实现代码: `src/stdlib/prelude.rs` (lines 485-501, Stage 94 标记)

## 预期/实际

- 预期测试数: 4
- 实际测试数: 4 ✓
- 覆盖率: prelude trait 编译路径 100%

## 测试矩阵更新

| 维度 | 增量 |
|------|------|
| 总测试数 | 5564 → 5568 |
| 失败数 | 0 |
| ignored | 9 |
