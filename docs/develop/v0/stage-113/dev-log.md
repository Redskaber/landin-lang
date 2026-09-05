# Stage 113 开发日志 — TD-LLVM-OBJ-EMIT-CRASH fix + TD-MONO-INFER fix

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.645.0 → v0.646.0 |
| 测试数 | 5673 → 5686 (+13 stage113 tests) |
| 失败数 | 0 → 0 |
| ignored | 9 |
| clippy warnings | 0 |
| LOC 变更 | +100 (src) + +300 (tests/docs) |

## 5W2H 根因分析

### WHAT (问题)
Stage 112 RCA found TD-LLVM-OBJ-EMIT-CRASH — deterministic SIGSEGV in
`LLVMTargetMachineEmitToFile` when emitting object files for IR with
`Operand::Constant` FnDef types that have non-empty inferred substs.

### WHY (根因)
The crash is NOT in the IR itself (llvm-as + llc both work). The crash
is in the LLVMSysEmitter's forward declaration path:

1. `writeback_fndef_substs` secondary pass propagates inferred substs
   into `Operand::Constant(c).ty` in Assign statements.
2. `codegen_operand` sees `FnDef(def_id, [i32])` → uses `mono_item_name`
   to compute specialized name (e.g., `@process_i32`).
3. LLVMSysEmitter's `interpret_adhoc` looks up `process_i32` in `fn_sigs`
   → NOT found (only base names like `landin_process` are in the map).
4. Falls back to variadic `i32 ()` forward declaration.
5. `codegen_mono_functions` later emits actual `process_i32` with correct
   sig `i32 (i32)` → type mismatch → old decl deleted + re-added.
6. References to the old (deleted) forward declaration become dangling
   pointers → SIGSEGV during LLVMTargetMachineEmitToFile.

### HOW (修复)
Three-part fix:

1. **`build_fn_sigs_map`** (function_sigs.rs): Add specialized function
   sigs to the fn_sigs map. For each MonoItem::Fn with non-empty substs,
   compute the specialized name and add it with the substituted signature.

2. **`writeback_fndef_substs`** (writeback.rs): Add secondary pass to
   propagate inferred substs from `local_decls[idx].ty` into Assign's
   `Operand::Constant(c).ty`.

3. **`codegen_from_mir`** (function.rs): Skip ALL prelude generic def
   bodies (not just those without MonoItem::Fn instantiation).

### WHO
- ARCH-A: 设计三部分修复方案
- DEV-A: 实施 + 调试 (用 addr2line + RUST_BACKTRACE)
- REV-A: 验证 0 回归
- QA-A: 13 个新测试

### WHEN
Stage 113 完成 → Stage 114 (重新添加 Debug impl bodies)

### WHERE
- `src/codegen/llvm/function_sigs.rs` — build_fn_sigs_map 添加 specialized sigs
- `src/mir/lower/writeback.rs` — writeback_fndef_substs secondary pass
- `src/codegen/function.rs` — codegen_from_mir skip ALL prelude generics
- `src/codegen/mod.rs` — build mono_names early for fn_sigs_map

### HOW MUCH
- 3 src files (~100 LOC) + 1 test file (13 tests) + 1 docs/tools/ + 1 debug script

## §3.2 验收

- `cargo fmt --check` ✓
- `cargo clippy --all-targets --features llvm-backend -- -D warnings` ✓ (0 warnings)
- `cargo test --release --features llvm-backend --lib` ✓ (898 tests, 0 failures)
- `cargo test --release --features llvm-backend --test all_tests` ✓ (4788 tests, 0 failures)
- Total: **5686 tests, 0 failures, 9 ignored**

## Stage Summary

- TD-LLVM-OBJ-EMIT-CRASH 修复完成
- TD-MONO-INFER 修复完成 (writeback secondary pass + skip rule)
- 根因: fn_sigs_map 缺少 specialized function sigs → variadic forward decl → type mismatch → SIGSEGV
- 修复: build_fn_sigs_map 添加 specialized sigs + writeback secondary pass + skip ALL prelude generics
- 13 个新测试覆盖正向/text IR/负向/边界
- 架构健康度: 9.9/10 (stable — 3 src files, 0 回归, 2 TD 修复)
- v0.646.0

## 下一步

- **Stage 114**: 重新添加 Debug impl bodies for i32/i64/bool/usize, 验证 100 次跑 0 SIGSEGV. TD-MONO-INFER + TD-LLVM-OBJ-EMIT-CRASH 都已修复, 预期 Debug impl 可安全添加.
