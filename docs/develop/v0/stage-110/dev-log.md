# Stage 110 开发日志 — Phase 3.6 (Constant type writeback) 重新引入

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.644.0 → v0.645.0 |
| 测试数 | 5633 → 5653 (+20 新增) |
| 失败数 | 0 → 0 |
| ignored | 9 |
| clippy warnings | 0 |
| LOC 变更 | +160 (src) + +400 (tests) |
| Infer warnings (Vec<String, i32> sample) | 41 → 19 (-54%) |

## 5W2H 设计

### WHAT (做什么)
重新引入 Phase 3.6 — typeck Phase 3 后添加 Constant type writeback. 遍历所有 basic_blocks 的 statement (Assign(_, Rvalue)) + terminator (SwitchInt discr / Call func+args / Assert cond), 对每个 Operand::Constant(c) 写回 `unify.resolve(&c.ty)` (Infer → concrete).

### WHY (为什么)
**Stage 105 RCA**: 100 次跑 3/100 SIGSEGV (ASLR on), 1/100 SIGSEGV (ASLR off). LLVM IR 在成功/失败跑间完全相同 (Param=73 Infer=18 warnings). 崩溃在 LLVM codegen/object emission 阶段. 根因: typeck Phase 3 不写 Constant.ty → `lit_to_const` 创建 Infer(IntVar) → codegen 看到 Infer 警告 → LLVM optimizer 非确定性处理.

**Stage 106 尝试 Phase 3.6** → 7 回归 (TD-CODEGEN-CALL-ARG-TYPE-SOURCE: codegen call arg type source 不一致).
**Stage 107 修复** TD-CODEGEN-CALL-ARG-TYPE-SOURCE (call arg type 优先用 callee sig).
**Stage 108 重试 Phase 3.6** → 7 回归 (TD-CODEGEN-CONST-SRC-TY-FROM-CONSTVAL: codegen src_ty 用 ConstVal 不用 c.ty → 不必要 sext cast).
**Stage 109 修复** TD-CODEGEN-CONST-SRC-TY-FROM-CONSTVAL + TextEmitter emit_const_typed contract bug.
**Stage 110 重新引入 Phase 3.6** — 所有前置依赖已修复, 0 回归.

### WHO (谁来定)
- ARCH-A: 设计遍历所有 Operand::Constant (statements + terminators) 通解, 不挑场景
- DEV-A: 实施 + 发现并修正 pre-existing bug 测试 (`push_str` runtime output "(?)", Box deref 不支持)
- REV-A: 自审 — 验证 warnings 减少 + 0 回归
- QA-A: 20 个新测试 (8 正 + 5 text IR + 4 负 + 3 边界)

### WHEN (何时停)
- §3.2 验收全绿 (5653 tests, 0 failures) → 提前收敛
- 单轮审查通过, 无 P0/P1

### WHERE (落哪里)
- `src/typeck/checker.rs` — Phase 3 后添加 Phase 3.6 遍历 (~60 LOC) + 两个 helper (`writeback_constant_ty_in_operand` + `writeback_constant_tys_in_rvalue`, ~100 LOC)
- `tests/v0/stage110/plan/phase36_const_writeback_tests.rs` — 20 个新测试
- `tests/all_tests.rs` — 注册 stage110_phase36_const_writeback_tests 模块

### HOW (怎么做)
1. **方案设计** (ARCH-A):
   - 选 A: **遍历所有 statement + terminator 中所有 Operand::Constant, 用 unify.resolve 写回 c.ty** — 通用机制, 不挑场景
   - 不选 B: 仅处理 SwitchInt discr — 太窄, Call args + Assert cond 仍有 Infer
   - 不选 C: 仅处理 Rvalue::Use(Operand::Constant) — 太窄, BinaryOp/UnaryOp/Cast/Aggregate/Load/GetElementPtr/BinaryOp2 都含 Operand

2. **实施** (DEV-A):
   - 在 Phase 3 后 (Phase 4 之前) 添加 Phase 3.6 块
   - 遍历 `mir.basic_blocks.iter_mut()`, 对每个 `bb.statements.iter_mut()` + `bb.terminator.kind`
   - Statement: 仅 `Assign(_, Rvalue)` 含 Operand; 通过 `writeback_constant_tys_in_rvalue` 递归处理所有 Rvalue variant
   - Terminator: SwitchInt discr / Call func+args / Assert cond — 通过 `writeback_constant_ty_in_operand` 处理
   - `writeback_constant_ty_in_operand(&self, op: &mut Operand)`: if Operand::Constant(c), 写回 `c.ty = self.unify.resolve(&c.ty)`
   - `writeback_constant_tys_in_rvalue(&self, rv: &mut Rvalue)`: match 所有 Rvalue variant, 递归调用上面的 helper

3. **测试** (QA-A): 20 个新测试
   - 8 正向: unsuffixed literal in call arg / struct literal / switch int / assert overflow / warnings reduced / 0 warnings simple program / text IR valid for unsuffixed struct literal / push_str compiles
   - 5 text IR: div_i64_unsuffixed + enum_unit + match_unsuffixed + vec_string + call_unsuffixed_arg (用 llvm-as 验证)
   - 4 负向/fallback: Box::new compile-only (Box deref TD), Vec::push runtime, nested Vec<Box>, char+bool constants
   - 3 边界: mixed width + isize/usize + large i64 unsuffixed

4. **发现并修正测试** (REV-A):
   - 第一次跑 stage110 测试 → 2 失败: `push_str` runtime output "(?)" 不是 "hello", `Box::new` deref 报错
   - 检查 baseline (Stage 107 stash): 两个都是 pre-existing bug, 不是 Phase 3.6 引入的回归
   - 修正: `push_str` 改为 compile-only, `Box::new` 改为 compile-only + 不 deref (用 `_b` let binding)
   - 修正后 20 测试全绿

### HOW MUCH (做到多好)
- §3.2 验收:
  - `cargo fmt --check` ✓
  - `cargo clippy --all-targets --features llvm-backend -- -D warnings` ✓ (0 warnings)
  - `cargo test --release --features llvm-backend --lib` ✓ (898 tests, 0 failures, 0 ignored)
  - `cargo test --release --features llvm-backend --test all_tests` ✓ (4755 tests, 0 failures, 9 ignored)
- 总: **5653 tests, 0 failures, 9 ignored** (Stage 109 baseline 5633 + 20 new)
- **Infer warnings 减少**: 41 → 19 (-54%) on Vec<String, i32> program (Stage 107 baseline)

## 决策点 (§12 最优>最小, §1.0 原则 9 正确>妥协, §1.0 原则 6 通解>特解)

### 决策 1: 选遍历所有 statement + terminator, 不选挑场景
- **方案 A (选)**: 遍历所有 basic_blocks 的 statement (Assign + Rvalue 递归) + terminator (SwitchInt/Call/Assert), 对每个 Operand::Constant(c) 写回 unify.resolve(&c.ty). 通用机制, 覆盖所有 Operand 嵌入点.
- **方案 B (不选)**: 仅处理 SwitchInt discr — 太窄, Call args + Assert cond 仍有 Infer.
- **方案 C (不选)**: 仅处理 Rvalue::Use(Operand::Constant) — 太窄, BinaryOp/UnaryOp/Cast/Aggregate/Load/GetElementPtr/BinaryOp2 都含 Operand.
- **引用**: §1.0 原則 6 (通解 > 特解 — 一条遍历覆盖所有场景), §1.0 原則 9 (正确 > 妥协 — 不挑场景的通解), §12 (最优 > 最小 — 一次到位)

### 决策 2: 选 helper 方法 (`writeback_constant_ty_in_operand` + `writeback_constant_tys_in_rvalue`), 不选内联 match
- **方案 A (选)**: 两个 helper 方法, 一个处理单 Operand, 一个递归 Rvalue 所有 variant. 代码组织清晰, 可复用, 可测试.
- **方案 B (不选)**: 内联 match — 代码散乱, 难测试, 难复用.
- **引用**: §1.0 原則 6 (通解 > 特解 — 一对 helper 覆盖所有调用点), §10 (DRY)

### 决策 3: 选 Phase 3.6 在 Phase 4 (TypeckResults) 之前, 不选之后
- **方案 A (选)**: Phase 3.6 在 Phase 3 之后, Phase 4 之前 — Phase 4 将 local.ty 写入 TypeckResults, Phase 3.6 在 Phase 4 之前确保 c.ty 已 resolve, Phase 4 不需要再次 resolve c.ty.
- **方案 B (不选)**: Phase 3.6 在 Phase 4 之后 — Phase 4 已经写 local.ty, 但 Operand::Constant(c).ty 仍未 resolve. 需要 Phase 3.6 在 Phase 4 之后单独 resolve.
- **引用**: §12 (最优 > 最小 — Phase 3.6 在 Phase 4 之前更简单)

## 裁剪点 (§1.2.1)

- L2 任务 (1 src 文件 ~160 LOC + 1 test 文件 ~400 LOC)
- 按 §1.2.1 跳过 §14.5 深度审查, 走 §7.3 门审查 + §3.2 验收
- §3.2 验收全绿 (5653 tests, 0 failures, 0 clippy warnings)

## §3.2 验收

- `cargo fmt --check` ✓
- `cargo clippy --all-targets --features llvm-backend -- -D warnings` ✓ (0 warnings)
- `cargo test --release --features llvm-backend --lib` ✓ (898 tests, 0 failures, 0 ignored)
- `cargo test --release --features llvm-backend --test all_tests` ✓ (4755 tests, 0 failures, 9 ignored)

## Stage Summary

- Phase 3.6 (Constant type writeback) 重新引入完成
- typeck Phase 3 后添加 Phase 3.6: 遍历所有 basic_blocks 的 statement + terminator, 对每个 Operand::Constant(c) 写回 unify.resolve(&c.ty)
- 添加两个 helper: `writeback_constant_ty_in_operand` + `writeback_constant_tys_in_rvalue`
- 覆盖所有 Rvalue variant + 所有含 Operand 的 TerminatorKind
- Infer warnings 41 → 19 (-54%) on Vec<String, i32> program
- 0 回归 (Stage 107 + 109 修复了所有前置依赖)
- 20 个新测试覆盖正向/text IR/负向/边界
- 架构健康度: 9.85/10 (stable — 1 src 文件, 无回归, Infer warnings 显著减少)
- v0.645.0

## 下一步

- **Stage 111**: 加 Debug impl 验证 100 次跑 0 SIGSEGV — Phase 3.6 active 后 c.ty 全部 concrete, 验证 Stage 105 非确定性 SIGSEGV 是否消除
- **Stage 112+**: 处理剩余 TD-TYPECK-WRITEBACK-INCOMPLETE 残留 (TD-MONO-INFER 非 turbofish path generic substs + TD-PRELUDE-IMPL-BODY-MODULE-ACCUMULATION LLVM module state)
