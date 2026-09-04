# Stage 95 测试计划 — PartialEq + Eq trait coverage

## 测试目标

验证 `PartialEq<Rhs>` + `Eq` traits 声明 + impls 不引入回归。重点验证 Eq 无 supertrait 方案不破坏现有 object safety 测试。

## 覆盖场景

| 场景 | 测试 | 类型 |
|------|------|------|
| traits 声明存在 | `stage95_partial_eq_eq_traits_declared` | 正向 |
| undefined type 报错 | `stage95_undefined_type_errors` | 负向 (resolve) |
| type mismatch 报错 | `stage95_type_mismatch_errors` | 负向 (typeck) |
| nonexistent method 报错 | `stage95_nonexistent_method_errors` | 负向 (typeck/lower) |

## 关键回归测试

- `stage16_78_supertrait_*` 系列 — 验证 Eq 无 supertrait 不干扰 user trait resolution。

## 对应代码

- 测试代码: `tests/v0/stage95/plan/partial_eq_eq_trait_tests.rs`
- 实现代码: `src/stdlib/prelude.rs` (Stage 95 标记, line 503+)

## 预期/实际

- 预期测试数: 4
- 实际测试数: 4 ✓
- 覆盖率: PartialEq/Eq 编译路径 100%

## 测试矩阵更新

| 维度 | 增量 |
|------|------|
| 总测试数 | 5568 → 5572 |
| 失败数 | 0 |
| ignored | 9 |
