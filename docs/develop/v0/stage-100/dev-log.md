# Stage 100 开发日志 — monomorphization 跳过 prelude generic function

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.638.0 → v0.639.0 |
| 测试数 | 5585 → 5592 (+7 stage100) |
| 失败数 | 0 → 0 |
| ignored | 9 |
| clippy warnings | 0 |
| LOC 变更 | 4 文件 src + 1 测试文件 (~150 LOC) |
| Param warnings | 1360 → 24 (-98%) |

## 修改文件

### 源文件 (4)
| 文件 | 变更 |
|------|------|
| `src/driver/mod.rs` | CompileResult 添加 `user_item_count: usize` 字段; `empty()` 初始化为 0 |
| `src/driver/compile_inner.rs` | CompileResult 构造时设置 user_item_count |
| `src/codegen/function.rs` | `codegen_from_mir` 接收 `user_item_count` + `collected_mono_items` 参数; 添加跳过逻辑; 添加 4 helper 函数 (`mir_body_contains_param_type`, `type_contains_param`, `statement_contains_param`, `operand_contains_param`, `terminator_contains_param`, `mono_items_contains_fn_for_def_id`) |
| `src/codegen/pipeline.rs` | 提前 collect_mono_items; 传 user_item_count + collected_mono_items 到 codegen_from_mir |

### 测试文件 (1)
| 文件 | 变更 |
|------|------|
| `tests/v0/stage100/plan/prelude_generic_skip_tests.rs` | 新建 — 7 tests (3 positive + 1 non-generic + 3 negative) |

### 其他
- `Cargo.toml`: 版本 → 0.639.0
- `tests/all_tests.rs`: 注册 stage100_prelude_generic_skip_tests

## 5W2H 根因修复

### WHAT (修复)
在 `codegen_from_mir` 中跳过 prelude generic function bodies **当且仅当** 没有 MonoItem::Fn 实例化引用它们。

### WHY (Layer 1 根因修复)
Stage 99 RCA 识别 4-layer 根因链:
- Layer 1: prelude generic methods 的 Param type 未解析 ← **本阶段修复**
- Layer 2: mir_type_to_emit_type Param fallback 到 i32 ← Stage 101
- Layer 3: LLVM module 全局变量累积 ← Stage 102
- Layer 4: LLVMSysEmitter::Drop 不释放 context ← Stage 102

本阶段修复 Layer 1: 跳过未实例化的 prelude generic function body emit。

### HOW (通解)
```
跳过条件: DefId >= user_item_count (prelude item)
       AND MIR body contains Param type (generic function)
       AND no MonoItem::Fn instantiation exists for this DefId
```

- 被实例化的 prelude generic function (如 `Box::new` 被 `Box::new(42i32)` 调用) 仍 emit generic def body (因为 codegen_operand 用 generic def 名引用 — Stage 101 修复)
- 未实例化的 prelude generic function (如 `Option::map` 未被调用) 跳过 emit, 减少 Param warnings

### 效果
- Param warnings: 1360 → 24 (-98%)
- Define count: 139 → 33 (prelude generic 未实例化的不 emit)
- 测试全绿: 898 lib + 4694 integration = 5592 tests, 0 failures, 9 ignored

## 决策点 (§12 最优>最小, §1.0 原则 6 通解>特解)

### 决策 1: 跳过条件用 MonoItem::Fn 实例化检查

**选择**: 只跳过**没有 MonoItem::Fn 实例化**的 prelude generic function。

**替代方案 (拒绝)**:
- 跳过所有 prelude generic function — 导致 `Box::new` 的 `store ptr @landin_Box_new` undefined reference (40 个测试失败)
- 在 codegen_operand 中修复 FnDef substs mangling — 正确但范围更大, 留给 Stage 101

**理由** (§1.0 原则 6 通解>特解, §1.0 原则 9 正确>妥协):
- 一条规则适用于所有 prelude items
- 双重检查 (DefId 边界 + MonoItem::Fn 实例化) 避免误跳过被调用的 generic function
- 与 rustc 设计一致 — generic function 定义不 emit, 只实例化版本 emit

### 决策 2: user_item_count 存到 CompileResult

**选择**: 在 CompileResult 添加 `user_item_count: usize` 字段。

**理由** (§1.0 原则 10 唯一可信数据源):
- user_item_count 已在 compile_inner.rs:79 计算
- codegen 不访问 HIR, 需通过 CompileResult 传递

### 决策 3: 提前 collect_mono_items 到 pipeline.rs

**选择**: 在 pipeline.rs 中提前调用 `collect_mono_items`, 传给 codegen_from_mir。

**理由** (§1.0 原则 10 唯一可信数据源):
- collect_mono_items 之前在 codegen_mono_functions 内部调用 (line 63)
- 现在 codegen_from_mir 也需要它, 提前到 pipeline 层避免重复计算
- mono_layouts 也用 collected_mono_items, 复用同一份数据

## 测试覆盖

| 测试 | 类型 | 验证 |
|------|------|------|
| `stage100_prelude_generic_instantiation_works` | 正向 | Box::new 编译通过 |
| `stage100_vec_new_instantiation_works` | 正向 | Vec::new 编译通过 |
| `stage100_option_map_instantiation_works` | 正向 | Option 使用编译通过 |
| `stage100_prelude_non_generic_function_works` | 正向 | String::from_str 编译通过 |
| `stage100_undefined_type_errors` | 负向 | undefined type 报错 |
| `stage100_type_mismatch_errors` | 负向 | type mismatch 报错 |
| `stage100_nonexistent_method_errors` | 负向 | nonexistent method 报错 |

## §3.2 验收

- `cargo fmt --check` ✓
- `cargo clippy --all-targets --features llvm-backend -- -D warnings` ✓ (0 warnings)
- `cargo test --release --features llvm-backend --lib` ✓ (898 tests, 0 failures, 0 ignored)
- `cargo test --release --features llvm-backend --test all_tests` ✓ (4694 tests, 0 failures, 9 ignored — stage100 7 tests included)

## 下一步

- **Stage 101**: 修复 `mir_type_to_emit_type` Param fallback (返回 Error 而非 i32) — Layer 2 根因
- **Stage 102**: LLVMSysEmitter ownership 重构 (Builder + Module 拆分) — Layer 3+4 根因
- **Stage 103**: 重新添加 Debug + PartialOrd impls (依赖 Stage 101+102 完成)
