# Stage 154 开发日志 — TD-DYN-LOCAL-FAT-PTR-COERCION 完整修复

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.677.0 → v0.678.0 |
| 测试数 | 6010 → 6019 (+9 new stage154 tests) |
| 失败数 | 0 |
| ignored | 12 |
| clippy warnings | 0 |
| fmt | clean |
| LOC 变更 | ~200 (6 src files) + ~250 (9 tests + 3 stage5 test files updated) |

## 5W2H

### WHAT
修复 `let g: &dyn Trait = &local; g.method()` 使用 GLOBAL `@.dynptr.Trait.Type` (data ptr = `@.data.Type = i8 0`) 而非 LOCAL fat pointer, 导致 method 访问 `self` 时读到 0 而非 local 的实际值.

### WHY
三个 bug 组合产生错误的运行时结果:
1. **types.rs**: `Ref(_, _, Dyn(_))` 走 `_` arm → `ptr_to(...)` (thin pointer, 8 bytes) 而非 fat pointer (16 bytes). alloca 丢失 vtable 指针.
2. **rvalue.rs/statement.rs**: `Rvalue::Ref` 返回 thin pointer, 不构造 fat pointer.
3. **operand.rs/aggregate.rs**: `emit_dyn_trait_method_call` 使用 GLOBAL dynptr symbol, 忽略 local fat pointer.

### 根因 (§2.2 根因思维)
- **症状**: `let g: &dyn Greeter = &e; g.greet()` 返回 0 (从 `@.data.English = i8 0` 读取) 而非 42 (从 local `e` 读取).
- **直接原因**: codegen 使用 GLOBAL dynptr `@.dynptr.Greeter.English` 进行 vtable dispatch, 该 global 的 data pointer 指向 `@.data.English` (1-byte placeholder `i8 0`), 而非 local `e` 的 alloca.
- **根因**: `Ref(Dyn)` 类型映射缺失 `Dyn` case → thin pointer 而非 fat pointer; fat pointer 构造缺失; dispatch 使用 global 而非 local.

### HOW (修复方案 — 三部分)

#### Part A: types.rs — 类型映射
添加 `TyKind::Dyn(_)` case 到 `Ref` inner match:
```rust
TyKind::Dyn(_) => EmitType::Struct(vec![EmitType::OpaquePtr, EmitType::OpaquePtr]),
```
使 `Ref(_, _, Dyn(_))` → fat pointer `{ptr, ptr}` (16 bytes). 同时更新 `_with_layouts` 和 `_with_layouts_and_mono` 两个变体.

#### Part B: statement.rs — fat pointer 构造
在 `codegen_statement` 的 Assign arm 中, 当 dest local 的类型是 `Ref(Dyn)` 且 rvalue 是 `Rvalue::Ref` 或 `Rvalue::Use(Copy/Move(place))` 时:
1. 从 place 的类型提取 concrete type 的 DefId
2. 通过 `type_name_by_def_id` 解析 trait_name 和 concrete_name
3. 构造 vtable symbol `@.vtable.{trait}.{concrete}`
4. 用 `emit_insertvalue` 构造 fat pointer `{ptr %local, ptr @.vtable}`

#### Part C: operand.rs + aggregate.rs — receiver value 使用
1. **Emitter trait**: `emit_dyn_trait_method_call` 参数从 `dynptr_symbol: &str` 改为 `receiver_value: &str` (可以是 `@global` 或 `%local`)
2. **TextEmitter**: GEP 直接使用 `receiver_value` (不再自动添加 `@` prefix)
3. **LLVMSysEmitter**: `@` 开头 → `LLVMGetNamedGlobal`; `%` 开头 → `self.lookup()`
4. **codegen_dyn_trait_call_direct**: 从 args[0] 提取 receiver local SSA value (当类型是 `Ref(Dyn)` 时使用 `%loc_{id}`), 否则回退到 global dynptr
5. **Call-site coercion (terminator.rs)**: 使用 local data pointer (`%loc_N` 或 load 后的 `%loc_N`) 而非 global `@.data.Type`

### 决策点 (§12 最优 > 最小, §1.0 原則 6/9/10)
1. **三部分同时修复** (§1.0 原則 9 正确 > 妥协) — Part A 单独会导致更多 verification error (type mismatch on store); Part A+B 单独会产生 silent wrong results (vtable dispatch 仍用 global). 必须三部分一起修复.
2. **Emitter trait API change** (§1.0 原則 6 通解 > 特解) — `emit_dyn_trait_method_call` 接收通用 `receiver_value: &str` 而非特定 `dynptr_symbol`, 一套 GEP pattern 同时处理 global 和 local.
3. **Call-site coercion 也使用 local data pointer** (§1.0 原則 9 正确 > 妥协) — 不只修 let-binding, 同时修 call-site coercion 使 `use_func(&local)` 也正确.

### 裁剪点 (§1.2.1)
L3 任务 (~200 LOC + 9 tests + 3 stage5 test files updated), 单轮收敛 (根因清晰: 三个 bug 组合, 一起修复). 跳过 §14.6 跨阶段验证 (单点修复, 无架构变化). 仍执行 §14.5 深度审查 (单轮).

### IR 验证

修复前 (`let g: &dyn Greeter = &e; g.greet()` where `e.val = 42`):
```llvm
%loc_5 = alloca ptr               ; BUG: thin pointer (should be {ptr,ptr})
store ptr %loc_3, ptr %loc_5       ; stores thin pointer
%v6 = getelementptr { ptr, ptr }, ptr @.dynptr.Greeter.English, i32 0, i32 0
%v7 = load ptr, ptr %v6            ; data ptr = @.data.English (i8 0)
%v8 = call i32 %v4(ptr %v7)        ; returns 0 (wrong)
```

修复后:
```llvm
%loc_5 = alloca { ptr, ptr }       ; CORRECT: fat pointer (16 bytes)
%v4 = insertvalue { ptr, ptr } undef, ptr %loc_3, 0     ; data ptr = local e
%v5 = insertvalue { ptr, ptr } %v4, ptr @.vtable.Greeter.English, 1
store { ptr, ptr } %v5, ptr %loc_5  ; stores fat pointer
%v6 = getelementptr { ptr, ptr }, ptr %loc_5, i32 0, i32 1
%v7 = load ptr, ptr %v6            ; vtable ptr (correct)
%v10 = getelementptr { ptr, ptr }, ptr %loc_5, i32 0, i32 0
%v11 = load ptr, ptr %v10          ; data ptr = local e (correct)
%v12 = call i32 %v8(ptr %v11)      ; returns 42 (correct)
```

### 发现的新 TD
- **TD-VTABLE-MISSING-DEFAULT-BODY** (P3, v0.16+): vtable 只包含 impl 提供的方法, 不包含 trait default body 方法. 调用 default body 方法 via dyn dispatch 会失败 (vtable index out of bounds 或 wrong method).
- **TD-DYN-TRAIT-METHOD-ARG-PLACEHOLDER** (P3, v0.16+ 部分修复): `codegen_dyn_trait_call_direct` 对 args[0] (receiver) 使用 `%arg0` 占位符. Stage 154 已修复 args[1:] 使用 `codegen_operand`, 但 args[0] 仍是占位符 (由 `emit_dyn_trait_method_call` 从 fat pointer 提取, 所以无影响).

### 下一步 (MUV)
Stage 155 候选:
- **TD-VTABLE-MISSING-DEFAULT-BODY** (P3, v0.16+): 修复 vtable 包含 default body 方法
- **TD-TYPECK-GENERIC-ARG-VALIDATION** (P3, v0.16+): 修复 typeck 的 turbofish arg 验证
- **TD-STDLIB-ITERATOR** (P3, v0.15+): 添加 Iterator trait + adapters 到 prelude

## §3.2 全套验收

| 命令 | 结果 |
|------|------|
| `cargo build --release --features llvm-backend` | success (34s) |
| `cargo check --features llvm-backend` | 0 errors, 0 warnings |
| `cargo fmt --check` | exit 0 (zero diff) |
| `cargo clippy --all-targets --features llvm-backend -- -D warnings` | 0 warnings (6s) |
| `cargo test --release --features llvm-backend` | 898 lib + 5121 integration = **6019 tests, 0 failures, 12 ignored** (193s) |

## 测试清单

| 测试 | 类型 | 描述 |
|------|------|------|
| `stage154_dyn_let_self_access` | 正向 | `let g: &dyn T = &e; g.greet()` 返回 self.val (42) |
| `stage154_dyn_let_method_returns_field` | 正向 | 多方法 + 字段访问 |
| `stage154_dyn_let_multiple_methods` | 正向 | &mut dyn + next() + current() |
| `stage154_regression_call_site_coercion` | 回归 | `use_greeter(&e)` (Stage 89/90) |
| `stage154_regression_dyn_let_no_self` | 回归 | method 不访问 self |
| `stage154_regression_dyn_param_function` | 回归 | `use_adder(&c, 23)` call-site + arg |
| `stage154_dyn_let_with_args` | 边界 | method with args beyond self |
| `stage154_dyn_let_chained_calls` | 边界 | 多个 dyn 引用 |
| `stage154_dyn_let_type_not_implementing_trait` | 负向 | typeck 拒绝不实现 trait 的类型 |
