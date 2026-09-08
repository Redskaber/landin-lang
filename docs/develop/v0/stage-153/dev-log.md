# Stage 153 开发日志 — TD-CALL-DEST-TYPE-SUBSTS 完整修复

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.676.0 → v0.677.0 |
| 测试数 | 6002 → 6010 (+8 new stage153 tests) |
| 失败数 | 0 |
| ignored | 12 |
| clippy warnings | 0 |
| fmt | clean |
| LOC 变更 | ~60 (call_dest_type) + ~300 (8 tests) |

## 5W2H

### WHAT
修复 `call_dest_type` (codegen/function.rs) 使用 `fn_sigs.get(&did).output` (未特化签名, 含 `Param(N)`) 而非调用点特化后的签名, 导致泛型函数/方法调用 destination local 的 alloca 大小错误 (Param → I32 fallback 而非具体类型 i64).

### WHY
Stage 18.107 已在 `terminator.rs:655-686` 实现了正确的 substitute 模式 — 从 `c.ty.kind = FnDef(did, substs)` 提取 substs 并 `substitute(sig.output, substs)` 后再 `mir_type_to_emit_type_with_layouts_and_mono`. 但 `call_dest_type` (Stage 14.36 引入) 没有同步此模式, 仅从 `c.val` 提取 DefId 并直接使用未特化的 `sig.output`. 这是 §1.0 原則 6 (通解 > 特解) 违反 — 同一 substitute 逻辑应统一应用.

### 根因 (§2.2 根因思维)
- **症状**: 泛型函数 `identity::<i64>(5000000000i64)` 的 destination local `%loc_3 = alloca i32` (4 bytes), 但实际 call 返回 i64 (8 bytes), 导致 store 溢出, 在 x86_64 Linux 上是潜在 UB.
- **直接原因**: `call_dest_type` 返回 `mir_type_to_emit_type_with_layouts_and_mono(&sig.output, ...)` 其中 `sig.output = Param(0)`, 走 Param fallback → I32.
- **根因**: `call_dest_type` 仅从 `c.val` (Uint/Int) 提取 DefId, 丢弃了 `c.ty` (FnDef(did, substs)) 携带的 call-site substs. 没有调用 `substitute(sig.output, callee_substs)` 特化输出类型.

### HOW (修复方案)
1. **提取 (callee_def_id, callee_substs) 元组**: 优先从 `c.ty.kind = FnDef(did, substs)` 提取 (携带 substs); 回退到 `c.val` (DefId only, empty substs). 同时处理 `Operand::Constant` 和 `Operand::Copy/Move` 两条路径.
2. **特化 sig.output**: 当 `callee_substs.is_empty()` 时直接用 `(*sig.output).clone()`; 否则用 `crate::mir::substitute::substitute(&sig.output, &callee_substs)`.
3. **传入特化后的 output**: `mir_type_to_emit_type_with_layouts_and_mono(&specialized_output, layouts, mono_layouts)`.

### 决策点 (§12 最优 > 最小, §1.0 原則 6/9/10)
1. **复用 `terminator.rs` 的成熟 substitute 模式** (§1.0 原則 6 通解 > 特解) — 不重新设计, 直接 mirror `terminator.rs:655-686` 的逻辑. 同一 substitute 路径适用于所有 generic callees.
2. **同时修复 Constant + Copy/Move 两条路径** (§1.0 原則 9 正确 > 妥协) — 不只修一个 case, 同时覆盖两种 Operand 形式. Closure case 也一并处理 (与 terminator.rs 一致).
3. **保留 c.val fallback** (§1.0 原則 10 唯一可信数据源) — c.ty 优先 (携带 substs), c.val 兼容旧 MIR (无 FnDef 类型). DefId 是唯一可信源, substs 来自 c.ty.

### 裁剪点 (§1.2.1)
L2 任务 (~60 LOC + 8 tests), 单轮收敛 (根因清晰: 复用成熟模式, 无新设计). 跳过 §14.6 跨阶段验证 (单文件修改, 无架构变化). 仍执行 §14.5 深度审查 (单轮).

### 验证 (IR 对比)
- **修复前** (`identity::<i64>(5000000000i64)`):
  ```llvm
  %loc_3 = alloca i32      ; BUG: 4 bytes for i64 value
  %v1 = call i64 @identity_i64(i64 5000000000)
  store i64 %v1, ptr %loc_3  ; writes 8 bytes to 4-byte alloca → UB
  ```
- **修复后**:
  ```llvm
  %loc_3 = alloca i64      ; CORRECT: 8 bytes for i64 value
  %v1 = call i64 @identity_i64(i64 5000000000)
  store i64 %v1, ptr %loc_3  ; 8 bytes to 8-byte alloca → safe
  ```
- **运行时**: 修复前后都返回 `5000000000` (因为 x86_64 stack alignment 偶然掩盖了 bug), 但修复后 IR 是正确的, 在其他平台/调用约定下不会出错.

### 发现的新 TD
- **TD-TYPECK-GENERIC-ARG-VALIDATION** (P3, v0.16+): `identity::<i64>(42i32)` 等 turbofish 指定的类型与实参类型不匹配时, typeck 静默接受而非报 type error. §1.0 原則 4 (报错 > 静默) 违反. 修复方案: 在 typeck 的 call arg check 中验证 turbofish substs 与实参类型一致, 不一致则报 E0308 (mismatched types).

### 下一步 (MUV)
Stage 154 候选:
- **TD-STDLIB-ITERATOR** (P3, v0.15+): 添加 Iterator trait + adapters (map, filter, collect) 到 prelude. Stage 153 修复了 call_dest_type 的特化路径, 为添加更多泛型 trait 方法 (如 `Iterator::map<B, F>`) 解锁了正确的 codegen 路径.
- **TD-TYPECK-GENERIC-ARG-VALIDATION** (P3, v0.16+): 修复 typeck 的 generic call arg 验证. 但需先评估是否阻断 Iterator 工作.
- **TD-TRAIT-METHOD-REMONO-LINK** (P3, v0.16+): vtable 引用 `landin_Iterator_Counter_next` 但函数未 emit (linker error). 与 Iterator trait 相关, 可能阻断 TD-STDLIB-ITERATOR.

## §3.2 全套验收

| 命令 | 结果 |
|------|------|
| `cargo clean` | success (1094 files removed, 595.5MiB) |
| `cargo build --release --features llvm-backend` | success (48s) |
| `cargo check --features llvm-backend` | 0 errors, 0 warnings (13s) |
| `cargo fmt --check` | exit 0 (zero diff) |
| `cargo clippy --all-targets --features llvm-backend -- -D warnings` | 0 warnings (20s) |
| `cargo test --release --features llvm-backend` | 898 lib + 5112 integration = **6010 tests, 0 failures, 12 ignored** (197s) |

## 测试清单

| 测试 | 类型 | 描述 |
|------|------|------|
| `stage153_generic_fn_return_param_i64` | 正向 | identity::<i64>(5e9) 返回 5e9 (> i32::MAX, 暴露 UB) |
| `stage153_generic_fn_return_param_i32` | 正向 | identity::<i32>(42) 返回 42 (sanity check) |
| `stage153_generic_method_return_param` | 正向 | trait method get::<i64>() 返回 8e9 (compile-fail acceptable) |
| `stage153_regression_iterator_sum_count` | 回归 | Stage 150 Iterator sum (count=5) |
| `stage153_regression_option_i32_round_trip` | 回归 | Stage 152 Option<i32> and_then |
| `stage153_regression_ufcs_basic` | 回归 | Stage 127 UFCS basic call |
| `stage153_generic_fn_no_turbofish_inferred` | 边界 | 推断 T (无 turbofish) |
| `stage153_generic_fn_inference_from_let_annotation` | 边界 | 推断 T (let 注解 + i64 arg) |
