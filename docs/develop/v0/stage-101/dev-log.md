# Stage 101 开发日志 — codegen_operand FnDef substs mangling

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.639.0 → v0.640.0 |
| 测试数 | 5592 → 5599 (+7 stage101) |
| 失败数 | 0 → 0 |
| ignored | 9 |
| clippy warnings | 0 |
| LOC 变更 | 5 文件 src + 1 测试文件 (~120 LOC) |
| Param warnings | 24 (unchanged — TD-MONO-INFER blocks further reduction) |

## 修改文件

### 源文件 (5)
| 文件 | 变更 |
|------|------|
| `src/codegen/operand.rs` | `codegen_operand` 接收 `mono_names` + `type_name_by_def_id` 参数; FnDef substs 非空时用 `mono_item_name` mangle 实例化名 |
| `src/codegen/function.rs` | `codegen_from_mir` + `codegen_function` + `codegen_synthesized_closure_functions` + `codegen_mono_functions` 接收 `mono_names` 参数; 内部传递 |
| `src/codegen/statement.rs` | `codegen_statement` + `emit_printf_call` 接收 `mono_names` + `type_name_by_def_id`; 内部传递给 `codegen_rvalue` + `codegen_operand` |
| `src/codegen/rvalue.rs` | `codegen_rvalue` 接收 `mono_names` + `type_name_by_def_id`; 内部传递给 `codegen_operand` |
| `src/codegen/terminator.rs` | `codegen_terminator` + `codegen_print_call` 接收 `mono_names` + `type_name_by_def_id`; 内部传递 |
| `src/codegen/pipeline.rs` | 提前 `build_mono_item_names`; 传 `mono_names` 给 `codegen_from_mir` + `codegen_synthesized_closure_functions` |

### 测试文件 (1)
| 文件 | 变更 |
|------|------|
| `tests/v0/stage101/plan/fndef_substs_mangling_tests.rs` | 新建 — 7 tests (3 positive + 4 negative) |

### 其他
- `Cargo.toml`: 版本 → 0.640.0
- `tests/all_tests.rs`: 注册 stage101_fndef_substs_mangling_tests
- `scripts/stage101_add_mono_names_params.py`: 批量参数添加脚本（保留）

## 5W2H 根因修复

### WHAT (修复)
1. `codegen_operand` 接收 `mono_names` + `type_name_by_def_id` 参数
2. FnDef substs 非空时用 `mono_item_name` mangle 实例化名 (e.g., `From::<i32>::from(42)` → `From_i32_from`)
3. FnDef substs 为空时 fallback 到 generic def name (e.g., `landin_Box_new`) — 由 `codegen_mono_functions` 处理实例化

### WHY (Layer 2 部分修复)
Stage 99 RCA Layer 2: codegen_operand FnDef substs 不 mangle 导致 generic def body 必须仍 emit (产生 Param warnings)。

本阶段建立了 mangling 基础设施（参数传递链 + mangle 逻辑），**turbofish path** 已正确 mangle。**非 turbofish path**（如 `Box::new(42i32)`）的 FnDef substs 为空 — 这是 TD-MONO-INFER (type inference back-propagation) 的领域，本阶段不修复。

### HOW (通解)
```
codegen_operand:
  if FnDef substs 非空 AND substs 全 concrete:
    lookup mono_names[MonoItem::Fn{def_id, substs}]
    if found: return "@" + specialized_name
    else: compute mono_item_name directly
  else (substs empty or non-concrete):
    return "@" + generic_def_name  // fallback
```

### 效果
- **turbofish path**: FnDef substs 正确 mangle 到实例化名 (新功能)
- **非 turbofish path**: 仍依赖 codegen_mono_functions 实例化 (与 Stage 100 行为一致)
- **Param warnings**: 24 (unchanged — 因 TD-MONO-INFER 未修，非 turbofish path 仍 emit generic def body)
- **测试全绿**: 898 lib + 4701 integration = 5599 tests, 0 failures, 9 ignored

## 决策点 (§12 最优>最小, §1.0 原则 6 通解>特解)

### 决策 1: 建立 mono_names 参数传递链

**选择**: 给 codegen_operand + codegen_function + codegen_statement + codegen_rvalue + codegen_terminator + codegen_print_call + emit_printf_call 都加 `mono_names` + `type_name_by_def_id` 参数。

**替代方案 (拒绝)**:
- ❌ 用 thread-local 全局变量 — 违反 §1.0 原则 3 (显式>隐式)
- ❌ 重建 mono_names 在 codegen_operand 内部 — 违反 §1.0 原则 10 (唯一可信数据源)

**理由** (§1.0 原则 10 唯一可信数据源, §1.0 原则 3 显式>隐式):
- mono_names 在 pipeline.rs 构建一次, 通过参数链显式传递
- 20+ 调用点都更新 — 工作量大但是正确的根因修复
- 与 rustc 设计一致 — codegen context 显式携带 mono data

### 决策 2: turbofish path mangle, 非 turbofish path fallback

**选择**: FnDef substs 非空且全 concrete 时 mangle; 否则 fallback 到 generic def name。

**理由** (§1.0 原则 9 正确>妥协):
- turbofish path (`From::<i32>::from(42)`) substs 已填 — 正确 mangle
- 非 turbofish path (`Box::new(42i32)`) substs 为空 — TD-MONO-INFER 跟踪
- 不强行 mangle 空 substs — 避免错误名 (e.g., `Box_new_` 无意义)

### 决策 3: 不修复 TD-MONO-INFER

**选择**: 不在本阶段修复 TD-MONO-INFER (type inference back-propagation 填充 FnDef substs)。

**理由** (§1.0 原则 9 正确>妥协, §12 最优>最小):
- TD-MONO-INFER 涉及 MIR lower + typeck 跨模块变更
- 单 stage 修复不完整会引入更多 bug
- 用户指示: 遇依赖缺失停止阉割版，转而分析根因
- 本阶段建立 mangling 基础设施, TD-MONO-INFER 修复后即可立即生效

## 测试覆盖

| 测试 | 类型 | 验证 |
|------|------|------|
| `stage101_turbofish_generic_instantiation_compiles` | 正向 | turbofish path `From::<i32>::from(42)` 编译通过 |
| `stage101_box_new_instantiation_compiles` | 正向 | 非 turbofish `Box::new(42i32)` 编译通过 (fallback path) |
| `stage101_prelude_non_generic_function_compiles` | 正向 | String::from_str 编译通过 |
| `stage101_undefined_type_errors` | 负向 | undefined type 报错 |
| `stage101_type_mismatch_errors` | 负向 | type mismatch 报错 |
| `stage101_nonexistent_method_errors` | 负向 | nonexistent method 报错 |
| `stage101_undefined_trait_errors` | 负向 | undefined trait 报错 |

## §3.2 验收

- `cargo fmt --check` ✓
- `cargo clippy --all-targets --features llvm-backend -- -D warnings` ✓ (0 warnings)
- `cargo test --release --features llvm-backend --lib` ✓ (898 tests, 0 failures, 0 ignored)
- `cargo test --release --features llvm-backend --test all_tests` ✓ (4701 tests, 0 failures, 9 ignored — stage101 7 tests included)

## 新发现 TD

### TD-MONO-INFER (P3, v0.11+) — type inference back-propagation for FnDef substs

**现象**: 非 turbofish path 的 generic call (e.g., `Box::new(42i32)`) 在 MIR lower 时 FnDef substs 为空。导致 codegen_operand 无法 mangle 到实例化名, generic def body 必须仍 emit (产生 Param warnings)。

**根因**: `lower_path_generic_args` 只看 turbofish (`<i32>`)，对 inference 推断的 substs 不填充。

**修复方案**: 在 typeck 完成后, 反向传播 inferred substs 到 FnDef 类型的 call sites。参考 rustc `InferCtxt` + `TypeVariable` 设计。

**影响**: 修复后 Param warnings 24 → 0, 可完全消除 TD-PRELUDE-IMPL-BODY-CODEGEN-CRASH Layer 2 残余。

## 下一步

- **Stage 102**: LLVMSysEmitter ownership 重构 (Builder + Module 拆分) — Layer 3+4
- **Stage 103**: 重新添加 Debug + PartialOrd impls (依赖 Stage 101+102 完成)
- **TD-MONO-INFER**: type inference back-propagation (P3, v0.11+) — 完全消除 Param warnings
