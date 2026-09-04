# Stage 103 测试计划 — lit_ty_resolution (Layer 3 partial fix)

## 测试目标

验证 Stage 103 resolve_lit_ty_from_expected 修复:
1. String::new() / Vec::new() / Box::new() 的 ptr field 类型正确解析为 usize
2. 负向测试覆盖错误恢复

## 覆盖场景

| 场景 | 测试 | 类型 |
|------|------|------|
| String::new ptr field | `stage103_string_new_ptr_field_type_resolved` | 正向 |
| Vec::new ptr field | `stage103_vec_new_ptr_field_type_resolved` | 正向 |
| Box::new ptr field | `stage103_box_new_field_type_resolved` | 正向 |
| undefined type 报错 | `stage103_undefined_type_errors` | 负向 |
| type mismatch 报错 | `stage103_type_mismatch_errors` | 负向 |
| nonexistent method 报错 | `stage103_nonexistent_method_errors` | 负向 |
| undefined trait 报错 | `stage103_undefined_trait_errors` | 负向 |

## 对应代码

- 测试代码: `tests/v0/stage103/plan/lit_ty_resolution_tests.rs`
- 实现代码: `src/mir/lower/expr_operand.rs` (lower_expr_to_operand + resolve_lit_ty_from_expected)

## 预期/实际

- 预期测试数: 7
- 实际测试数: 7 ✓

## 测试矩阵更新

| 维度 | 增量 |
|------|------|
| 总测试数 | 5606 → 5613 (+7 stage103) |
| 失败数 | 0 |
| ignored | 9 |

## 已知限制

加 Debug impl 后 cargo test 仍有 5 失败 — Param warnings from generic prelude methods (Vec::push<T>) 仍存在。TD-MONO-INFER (P3, v0.11+) 跟踪 type inference back-propagation 完全修复。
