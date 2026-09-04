# Stage 101 测试计划 — FnDef substs mangling

## 测试目标

验证 Stage 101 codegen_operand FnDef substs mangling 修复:
1. turbofish path generic instantiation 编译通过 (FnDef substs 非空时正确 mangle)
2. 非 turbofish path generic instantiation 编译通过 (fallback 到 generic def name)
3. prelude non-generic function 仍工作
4. 负向测试覆盖错误恢复

## 覆盖场景

| 场景 | 测试 | 类型 |
|------|------|------|
| turbofish `From::<i32>::from(42)` | `stage101_turbofish_generic_instantiation_compiles` | 正向 |
| `Box::new(42i32)` 非 turbofish | `stage101_box_new_instantiation_compiles` | 正向 |
| `String::from_str` non-generic | `stage101_prelude_non_generic_function_compiles` | 正向 |
| undefined type 报错 | `stage101_undefined_type_errors` | 负向 |
| type mismatch 报错 | `stage101_type_mismatch_errors` | 负向 |
| nonexistent method 报错 | `stage101_nonexistent_method_errors` | 负向 |
| undefined trait 报错 | `stage101_undefined_trait_errors` | 负向 |

## 关键回归测试

- 所有 stage18_189_box_new_as_str_tests — Box::new 非 turbofish 实例化
- 所有 stage18_195_vec_mvp_tests — Vec::new 非 turbofish 实例化
- 所有 stage18_197_vec_push_tests — Vec::push 非 turbofish 实例化
- stage65_prelude_macro_timing_tests — prelude macro timing

## 对应代码

- 测试代码: `tests/v0/stage101/plan/fndef_substs_mangling_tests.rs`
- 实现代码: `src/codegen/operand.rs` (codegen_operand + FnDef substs mangle), `src/codegen/function.rs`, `src/codegen/statement.rs`, `src/codegen/rvalue.rs`, `src/codegen/terminator.rs`, `src/codegen/pipeline.rs`

## 预期/实际

- 预期测试数: 7
- 实际测试数: 7 ✓
- Param warnings: 24 (unchanged — TD-MONO-INFER blocks further reduction)

## 测试矩阵更新

| 维度 | 增量 |
|------|------|
| 总测试数 | 5592 → 5599 (+7 stage101) |
| 失败数 | 0 |
| ignored | 9 |

## 已知限制

非 turbofish path 的 generic call (e.g., `Box::new(42i32)`) 的 FnDef substs 仍为空 — TD-MONO-INFER 跟踪 type inference back-propagation 修复 (P3, v0.11+)。修复后 Param warnings 24 → 0。
