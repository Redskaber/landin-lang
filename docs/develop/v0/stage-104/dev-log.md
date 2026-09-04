# Stage 104 开发日志 — TD-MONO-INFER 根因分析 (writeback_fndef_substs 已存在)

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.642.0 (无版本变更 — RCA only) |
| 测试数 | 5613 (898 lib + 4715 integration) |
| 失败数 | 0 → 0 |
| ignored | 9 |
| clippy warnings | 0 |
| LOC 变更 | 0 src (RCA + 注释更新) + 1 helper visibility change |

## 修改文件

### 源文件 (1)
| 文件 | 变更 |
|------|------|
| `src/mir/lower/expr_operand.rs` | `collect_param_bindings` 从 `fn` 改为 `pub(crate) fn` (为 Stage 104 实验准备, 保留) |
| `src/mir/lower/expr_variants.rs` | 添加 Stage 104 注释 (writeback_fndef_substs 已处理, 不在 MIR lower 推断) |
| `src/stdlib/prelude.rs` | 更新 Debug trait 注释 (Stage 104 RCA 结论) |

## 5W2H 根因分析

### WHAT (发现)
TD-MONO-INFER 的 type inference back-propagation **已由 `writeback_fndef_substs` 实现** (driver/compile_inner.rs:1017, Stage 18.102)。该 pass 在 MIR lower + typeck 之后运行, 从 arg types 推断 FnDef substs 并写入 local_decls。

### WHY (Stage 104 实验结论)
Stage 104 尝试在 `lower_call_expr` 中手动推断 substs (MIR lower 阶段), 但导致 12 个测试失败 — typeck 在 `check_terminator` 中看到 `FnDef(105, [i32])` (手动推断) vs `FnDef(105, [])` (其他来源), 触发 false mismatch。

根因: typeck 在 `writeback_fndef_substs` 之前运行, 此时 FnDef substs 仍为空。typeck 的 `unify` 对 `FnDef` 相同 DefId 返回 Ok (line 814), 但 `check_terminator` 的 arg unify (`unify(input_ty, arg_ty)`) 中 `input_ty` 来自 `fn_sigs` (含 Param), `arg_ty` 来自 local_decls (可能被手动推断影响)。

### HOW (通解 — 已存在)
```
driver pipeline:
  1. MIR lower → FnDef substs 为空 (无 turbofish)
  2. typeck → unify arg types with sig inputs (Param unifies with anything)
  3. writeback_type_propagation → 传播 concrete types
  4. writeback_fndef_substs → 从 arg types 推断 FnDef substs, 写入 local_decls
  5. writeback_type_propagation (re-run) → 传播 substituted return types
  6. codegen → 读取 local_decls (已含 inferred substs)
```

### 效果
- **不加 Debug impl**: 50 次跑 String::new + push_str 全绿 (0 failures)
- **加 Debug impl**: 100 次跑 2 失败 (Param warnings from generic def body emit 仍存在)
- **Param warnings**: 24 → 仍 24 (generic def body 内部 Param 未被 substitute)

### 残留问题
generic def body (如 `landin_Box_new`) 仍被 codegen_from_mir emit, 其内部 Param types fallback 到 i32。原因是 codegen_from_mir (Stage 100) 只跳过**无 MonoItem::Fn 实例化**的 prelude generic, 但被实例化的 generic def body 仍 emit (因为 codegen_operand 可能引用 generic def 名)。

Stage 101 的 codegen_operand FnDef substs mangling 在 turbofish path 工作, 但非 turbofish path 的 FnDef substs 在 codegen 时仍为空 (writeback 更新了 local_decls, 但 codegen 读取的是 MIR body 的 Call terminator func operand, 该 operand 引用 local_decls — 应该已含 inferred substs)。

需进一步调查 codegen 读取 FnDef substs 的时机是否在 writeback 之后。

## 决策点 (§12 最优>最小, §1.0 原则 9 正确>妥协)

### 决策 1: 不在 MIR lower 中手动推断 substs

**选择**: 移除 Stage 104 手动推断代码, 保留 `writeback_fndef_substs` 作为唯一推断点。

**理由** (§1.0 原则 9 正确>妥协, §12 最优>最小):
- 手动推断在 typeck 之前运行, 导致 false mismatch
- `writeback_fndef_substs` 在 typeck 之后运行, 正确避免 mismatch
- 一个推断点 (writeback) 优于两个 (MIR lower + writeback)

### 决策 2: 保留 collect_param_bindings 可见性变更

**选择**: 保留 `collect_param_bindings` 从 `fn` 改为 `pub(crate) fn`。

**理由** (§1.0 原则 6 通解>特解):
- 未来可能需要从其他模块调用 (如 codegen 或 driver)
- 可见性变更是安全的 (不破坏现有 API)

## §3.2 验收

- `cargo fmt --check` ✓
- `cargo clippy --all-targets --features llvm-backend -- -D warnings` ✓ (0 warnings)
- `cargo test --release --features llvm-backend --lib` ✓ (898 tests, 0 failures, 0 ignored)
- `cargo test --release --features llvm-backend --test all_tests` ✓ (4715 tests, 0 failures, 9 ignored)
- 50 次手动跑 String::new + push_str 全绿 (0 failures)

## 下一步

- **Stage 105**: 调查 codegen 读取 FnDef substs 时机 — 确认 writeback 更新的 local_decls 是否在 codegen 时可见
- **Stage 106**: 如果 codegen 已读取 inferred substs, 调查为何 generic def body 仍 emit (可能需修改 Stage 100 跳过条件)
- **Stage 107**: 重新添加 Debug + PartialOrd impls (依赖 Stage 105-106 完成)
