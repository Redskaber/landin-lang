# Stage 100 测试计划 — prelude generic skip

## 测试目标

验证 Stage 100 monomorphization 跳过 prelude generic function bodies 修复:
1. prelude generic instantiation 仍工作 (Box::new, Vec::new, Option 使用)
2. prelude non-generic function 仍工作 (String::from_str)
3. 负向测试覆盖错误恢复

## 覆盖场景

| 场景 | 测试 | 类型 |
|------|------|------|
| Box::new generic instantiation | `stage100_prelude_generic_instantiation_works` | 正向 |
| Vec::new generic instantiation | `stage100_vec_new_instantiation_works` | 正向 |
| Option 使用 generic type | `stage100_option_map_instantiation_works` | 正向 |
| String::from_str non-generic | `stage100_prelude_non_generic_function_works` | 正向 |
| undefined type 报错 | `stage100_undefined_type_errors` | 负向 |
| type mismatch 报错 | `stage100_type_mismatch_errors` | 负向 |
| nonexistent method 报错 | `stage100_nonexistent_method_errors` | 负向 |

## 关键回归测试

- 所有 stage18_189_box_new_as_str_tests — Box::new 实例化
- 所有 stage18_195_vec_mvp_tests — Vec::new 实例化
- 所有 stage18_197_vec_push_tests — Vec::push 实例化
- stage65_prelude_macro_timing_tests — prelude macro timing

## 对应代码

- 测试代码: `tests/v0/stage100/plan/prelude_generic_skip_tests.rs`
- 实现代码: `src/codegen/function.rs` (codegen_from_mir + helpers), `src/codegen/pipeline.rs` (提前 collect_mono_items), `src/driver/mod.rs` (user_item_count 字段), `src/driver/compile_inner.rs`

## 预期/实际

- 预期测试数: 7
- 实际测试数: 7 ✓
- Param warnings: 1360 → 24 (-98%)

## 测试矩阵更新

| 维度 | 增量 |
|------|------|
| 总测试数 | 5585 → 5592 (+7 stage100) |
| 失败数 | 0 |
| ignored | 9 |
