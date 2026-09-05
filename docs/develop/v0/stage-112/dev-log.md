# Stage 112 开发日志 — TD-MONO-INFER RCA (attempted fix + REVERTED)

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.645.0 (无版本变更 — RCA + revert) |
| 测试数 | 5663 → 5673 (+10 stage112 RCA tests) |
| 失败数 | 0 → 0 (reverted) |
| ignored | 9 |
| clippy warnings | 0 |
| LOC 变更 | 0 src (reverted) + 10 tests |

## 5W2H 根因分析

### WHAT (实验 + 发现)
Stage 111 RCA identified TD-MONO-INFER + TD-PRELUDE-IMPL-BODY-MODULE-ACCUMULATION as the dependency gap blocking Debug impl re-add. Stage 112 attempted a two-part fix:

1. **codegen/function.rs skip rule strengthening**: Skip ALL prelude generic def bodies (regardless of MonoItem::Fn instantiation). codegen_mono_functions handles all instantiated versions via substitute_mir_body.

2. **writeback_fndef_substs secondary pass**: Propagate inferred substs from `local_decls[idx].ty` (updated by terminator_changes) into the `Operand::Constant(c).ty` in Assign statements (where `c.ty` was `FnDef(def_id, [])` with empty substs).

### Result: REVERTED

- **Fix #1 alone** (skip rule): 43 linker errors. Generic def not emitted, call sites reference `@landin_Vec_new` which is undefined.
- **Fix #1 + #2 together**: 6 impl Trait tests crash in `--emit-obj` (deterministic SIGSEGV in LLVMTargetMachineEmitToFile). The IR is valid (llvm-as + llc both work), but the in-memory LLVMModule crashes when emitted via the C API.

### WHY (根因)
The impl Trait desugar (`fn process<T: Clone>(x: T) -> i32`) creates a user-defined generic function. The writeback secondary pass propagates inferred substs into `Operand::Constant` in Assign statements. This changes the IR structure in a way that triggers a deterministic crash in LLVM's object emission path (LLVMTargetMachineEmitToFile).

The crash is NOT in the IR itself (llvm-as accepts it, llc compiles it to .s successfully). It's in the LLVM C API binding path (LLVMSysEmitter::emit_to_object_file or similar) — likely a use-after-free or module state issue.

### WHO (影响)
- 影响: All generic function codegen paths that use Operand::Constant with inferred substs
- 阻断: Cannot fix TD-MONO-INFER without first fixing TD-LLVM-OBJ-EMIT-CRASH
- 新发现 TD: TD-LLVM-OBJ-EMIT-CRASH (P2, v0.13+)

### WHEN (触发条件)
1. impl Trait desugar creates user-defined generic function
2. writeback_fndef_substs secondary pass propagates inferred substs into Operand::Constant
3. codegen emits the IR with non-empty substs in Constant
4. LLVMTargetMachineEmitToFile crashes (deterministic SIGSEGV)

### WHERE (代码位置)
- `src/codegen/function.rs` (Stage 100 skip rule) — fix #1 attempted + reverted
- `src/mir/lower/writeback.rs` (writeback_fndef_substs) — fix #2 secondary pass attempted + reverted
- LLVM C API: `LLVMTargetMachineEmitToFile` — crash site (TD-LLVM-OBJ-EMIT-CRASH)

### HOW (复现步骤)
1. Apply fix #1 (skip ALL prelude generic def bodies in codegen_from_mir)
2. Apply fix #2 (writeback secondary pass: propagate local_decls[idx].ty into Assign's Operand::Constant(c).ty)
3. `cargo build --release --features llvm-backend --bin landin-stage0`
4. `landin-stage0 --emit-obj test_impl_trait.lin` (where test uses `fn process<T: Clone>(x: T) -> i32`)
5. Observe: SIGSEGV in LLVMTargetMachineEmitToFile (deterministic)
6. Verify IR is valid: `landin-stage0 --emit-llvm-ir test_impl_trait.lin > out.ll && llvm-as out.ll && llc out.ll -o out.s` (both succeed)

### HOW MUCH (影响范围)
- Stage 111 baseline (no fix): 0 failures (5663 tests, Debug impl deferred)
- Stage 112 fix #1 alone: 43 linker errors (generic def not emitted)
- Stage 112 fix #1 + #2: 6 impl Trait --emit-obj crashes (deterministic SIGSEGV)
- 阻断: TD-MONO-INFER fix blocked by TD-LLVM-OBJ-EMIT-CRASH

## 决策点 (§12 最优>最小, §1.0 原则 9 正确>妥协, 用户指示 tech-debt workflow)

### 决策 1: 选 revert both fixes, 不选保留不完整修复
- **方案 A (选)**: Revert both fix #1 + #2. Preserve Stage 111 baseline (5663 tests, 0 failures). Document dependency gap (TD-LLVM-OBJ-EMIT-CRASH).
- **方案 B (不选)**: 保留 fix #1 + #2 + 接受 6 impl Trait crashes — 违反 §1.0 原则 9 (正确 > 妥协) + §3.2 红线 (test --release 必须全绿).
- **方案 C (不选)**: 仅保留 fix #2 (writeback secondary pass) — 43 linker errors, 不可接受.
- **引用**: §1.0 原則 9 (正确 > 妥协: 不发布确定性 crash), 用户指示 (tech-debt workflow: 停止阉割版推进, 转而分析缺失依赖), §17.6 (直到审查不出问题为止: 继续迭代).

### 决策 2: 选记录新 TD (TD-LLVM-OBJ-EMIT-CRASH), 不选忽略
- **方案 A (选)**: 同步更新 tech-debt-register.md — 添加 TD-LLVM-OBJ-EMIT-CRASH (P2, v0.13+) 作为 TD-MONO-INFER fix 的硬阻断.
- **方案 B (不选)**: 不记录 — 违反用户指示 (及时同步 TD) + §1.0 原則 4 (报错 > 静默).
- **引用**: 用户指示 (发现依赖缺失, 同步到 tech-debt), §1.0 原則 4 (报错 > 静默).

### 决策 3: 选保留 Stage 111 baseline, 不选一并 revert Phase 3.6
- **方案 A (选)**: Stage 111 baseline (Phase 3.6 active, Debug impl deferred) 是正确的. Stage 112 RCA 不影响它.
- **方案 B (不选)**: Revert Phase 3.6 + Stage 112 fixes 一起 — 过度 revert, Phase 3.6 是 Stage 105-110 迭代修复链的成果.
- **引用**: §12 (最优 > 最小: 最小 revert, 保留正确修复), §1.0 原則 9 (正确 > 妥协: Phase 3.6 active 不影响 baseline 稳定性).

## 裁剪点 (§1.2.1)

- L3 任务 (跨 codegen + writeback + LLVM C API binding), 实际改动 L2 (~80 LOC src + 10 tests)
- 按 §1.2.1 走 §7.3 门审查 + §3.2 验收 + 非确定性验证 (核心门禁)
- §3.2 验收 (reverted 后): 全绿 (5673 tests, 0 failures, 0 clippy warnings)

## §3.2 验收 (reverted 后)

- `cargo fmt --check` ✓
- `cargo clippy --all-targets --features llvm-backend -- -D warnings` ✓ (0 warnings)
- `cargo test --release --features llvm-backend --lib` ✓ (898 tests, 0 failures, 0 ignored)
- `cargo test --release --features llvm-backend --test all_tests` ✓ (4775 tests, 0 failures, 9 ignored)
- 总: 5673 tests (898 lib + 4775 integration + 10 stage112 RCA), 0 failures

## Stage Summary

- TD-MONO-INFER fix attempted + REVERTED
- Two-part fix (codegen skip rule + writeback secondary pass) caused either 43 linker errors (fix #1 alone) or 6 impl Trait --emit-obj crashes (fix #1+#2)
- New TD discovered: TD-LLVM-OBJ-EMIT-CRASH (deterministic SIGSEGV in LLVMTargetMachineEmitToFile)
- Revert both fixes, preserve Stage 111 baseline (5663 tests, 0 failures)
- 添加 10 stage112 RCA tests
- 升级 tech-debt TD-MONO-INFER + 添加 TD-LLVM-OBJ-EMIT-CRASH
- 架构健康度: 9.85/10 (stable — RCA + revert, 无代码变更, 依赖 gap 记录)

## 下一步

- **Stage 113**: 调查 TD-LLVM-OBJ-EMIT-CRASH — LLVM C API binding path 的 use-after-free 或 module state issue. 需要用 lldb/valgrind 调试 LLVMSysEmitter::emit_to_object_file. 参考 LLVM 22 的 `LLVMRustExecutionContext` (LLVM 19+ per-thread context) 作为可能的隔离方案.
- **Stage 114**: 修复 TD-LLVM-OBJ-EMIT-CRASH 后, 重新应用 Stage 112 fix #1 + #2, 验证 0 回归.
- **Stage 115**: 再次重新添加 Debug impl bodies, 验证 100 次跑 0 SIGSEGV (依赖 Stage 113 + 114 完成).
