# Stage 109 开发日志 — TD-CODEGEN-CONST-SRC-TY-FROM-CONSTVAL 修复

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.643.0 → v0.644.0 |
| 测试数 | 5613 → 5633 (+20 新增) |
| 失败数 | 0 → 0 |
| ignored | 9 |
| clippy warnings | 0 |
| LOC 变更 | +120 (src) + +400 (tests) + 文档更新 |

## 5W2H 设计

### WHAT (做什么)
修复 `TD-CODEGEN-CONST-SRC-TY-FROM-CONSTVAL` — codegen Stage 14.64 cast 逻辑的 `src_ty` 派生来源错误。

### WHY (为什么)
**Stage 108 RCA**: Phase 3.6 (Constant type writeback) 将 `Constant.ty` 从 `Infer(IntVar)` resolve 为 concrete type (I32 或 I64). 但 codegen 的 `src_ty` 派生自 `ConstVal` 值大小 (`42` fits in i32 → src=I32), 而 `target_ty` 来自 `c.ty` (可能是 I64) → `src != target` → 不必要 `sext i32 42 to i64` cast → 7 个 codegen 测试回归.

**根因**: `src_ty` 应该用 `c.ty` (resolve 后的 concrete type), 不用 `ConstVal` 值大小。

### WHO (谁来定)
- ARCH-A: 设计 emit_const_typed 直接 emit 方案 (而非改 src_ty 派生 — 后者会因 emit_const 仍 emit i32 导致 LLVM verify 失败)
- DEV-A: 实施 + 发现并修复 TextEmitter contract bug
- REV-A: 自审 — 发现并修复 21 text IR 测试失败 (双类型前缀 bug)
- QA-A: 20 个新测试 (8 正 + 5 text IR + 4 负 + 3 边界)

### WHEN (何时停)
- §3.2 验收全绿 (5633 tests, 0 failures) → 提前收敛
- 单轮审查通过, 无 P0/P1

### WHERE (落哪里)
- `src/codegen/operand.rs` — Stage 14.64 cast 块, 添加 concrete int-like c.ty 检测 + emit_const_typed 路径
- `src/codegen/text/arithmetic.rs` — TextEmitter emit_const_typed 返回 raw value (无 type prefix), 对齐 LLVM emitter contract
- `tests/v0/stage109/plan/const_src_ty_tests.rs` — 20 个新测试
- `tests/all_tests.rs` — 注册 stage109_const_src_ty_tests 模块

### HOW (怎么做)
1. **方案设计** (ARCH-A):
   - 选 A: `emit_const_typed` 直接 emit — 一步到位, LLVM backend 用 LLVMConstInt 创建 concrete type constant, TextEmitter 返回 raw value
   - 不选 B: 仅改 `src_ty` 派生 — 不够, `emit_const` 仍 emit `i32 42` (LLVMConstInt(I32Type, 42)), 但 src_ty=I64 → LLVM verify 失败 (i32 constant where i64 expected)
   - 不选 C: 跳过特定 width cast — 太激进 (Stage 108 试过 → 301 回归)

2. **实施** (DEV-A):
   - 在 cast 块开头添加 `concrete_int_ty_109` 检测: `c.ty.kind` 为 `Int(_)/Uint(_)/Bool/Char` → 用 `emit_const_typed(n_val, &int_emit_ty)` 直接 emit
   - 否则 fallback 到原 ConstVal 路径 (preserves Stage 107 behavior — 当 c.ty 为 Infer 时无变化)

3. **发现并修复 TextEmitter contract bug** (REV-A):
   - 第一次 build + test → 21 个 text IR 测试失败, 错误模式 `store i64 i64 1, ptr %loc_3` (双类型前缀)
   - 根因: TextEmitter `emit_const_typed` 返回 `"i64 1"` (typed literal), 但 consumer (`emit_store`, `emit_icmp` 等) 已经 prepend 类型前缀
   - 这是 **Stage 18.287 遗留 bug** — 之前没有 text IR 测试 exercise 这些路径, 所以未被发现
   - 修复: TextEmitter `emit_const_typed` 返回 raw value (`"1"` 而非 `"i64 1"`), 对齐 LLVM emitter 的 contract (返回 SSA name `%v3`, 无 type prefix)
   - 修复后 21 测试全绿, baseline text IR 也修复 (`icmp eq i64 2, i64 0` → `icmp eq i64 2, 0`)

4. **测试** (QA-A): 20 个新测试
   - 8 正向: i64/i32/usize/i8/i16/u8/u32/large_i64/bool/char constant 在 call arg + struct literal 中
   - 5 text IR: div_i64 + struct_i64 + enum_unit + zst_struct + bool_to_int (用 llvm-as 验证)
   - 4 负向/fallback: Infer c.ty 路径 (unsuffixed literal) — 验证 Stage 107 behavior preserved
   - 3 边界: i128 + isize + mixed-width

### HOW MUCH (做到多好)
- §3.2 验收:
  - `cargo fmt --check` ✓
  - `cargo clippy --all-targets --features llvm-backend -- -D warnings` ✓ (0 warnings)
  - `cargo test --release --features llvm-backend --lib` ✓ (898 tests, 0 failures, 0 ignored)
  - `cargo test --release --features llvm-backend --test all_tests` ✓ (4735 tests, 0 failures, 9 ignored)
- 总: 5633 tests, 0 failures, 9 ignored (Stage 107 baseline 5613 + 20 new)

## 决策点 (§12 最优>最小, §1.0 原则 9 正确>妥协)

### 决策 1: 选 `emit_const_typed` 直接 emit, 不选改 `src_ty` 派生
- **方案 A (选)**: 当 c.ty 为 concrete int-like type 时, 用 `emit_const_typed(value, &c.ty_emit_type)` 直接 emit — LLVM backend 用 LLVMConstInt 创建正确类型 constant, TextEmitter 返回 raw value. 跳过 sext/trunc cast 完全.
- **方案 B (不选)**: 仅改 `src_ty` 派生 (从 `c.ty` 而非 ConstVal). 但 `emit_const` 仍 emit i32 constant (因为 `42 <= i32::MAX`), `src_ty=I64`, `target_ty=I64` → 不 cast → return raw (which is `i32 42`) → LLVM verify 失败 (i32 constant in i64 context).
- **方案 C (不选)**: Stage 108 试过 "skip cast for I32↔I64↔I128 when value fits" — 太激进, 301 个回归 (其他 cast 也被跳过).
- **引用**: §12 (最优>最小 — 根因修复), §1.0 原則 9 (正确>妥协 — 不在阉割版上妥协), §1.0 原則 6 (通解>特解 — 一条路径覆盖所有 concrete int-like type)

### 决策 2: 选 TextEmitter contract 对齐, 不选 per-caller workaround
- **方案 A (选)**: 修改 TextEmitter `emit_const_typed` 返回 raw value (`"1"` 而非 `"i64 1"`), 对齐 LLVM emitter contract (返回 SSA name `%v3`, 无 type prefix).
- **方案 B (不选)**: 在 codegen_operand 中检测是否为 TextEmitter 并分支处理 — 太脆弱, 每个新 caller 都要重新处理 contract.
- **方案 C (不选)**: 修改所有 consumer (`emit_store`, `emit_icmp` 等) 不 prepend 类型前缀 — 但这是 LLVM IR 语法要求 (`store i64 <val>, ptr <ptr>` 必须有类型), 不可行.
- **引用**: §1.0 原則 6 (通解>特解 — 一条 contract 覆盖两个 emitter), §1.0 原則 9 (正确>妥协 — 修复 contract bug, 非 workaround), §17.6 (直到审查不出问题为止 — 21 失败触发的深挖)

### 决策 3: 选 fallback 路径保留, 不选强制要求 c.ty 为 concrete
- **方案 A (选)**: 当 c.ty 为 Infer/Param/etc. (Phase 3.6 未应用) 时, fallback 到原 ConstVal 路径 — 透明, 无行为变化.
- **方案 B (不选)**: 强制要求 c.ty 为 concrete, 否则 panic. 但当前 Phase 3.6 未应用 (Stage 108 revert), 所有 unsuffixed literal 的 c.ty 都是 Infer → 大量代码崩溃.
- **引用**: §1.0 原則 9 (正确>妥协 — Stage 110 重新引入 Phase 3.6 后, c.ty 自然变 concrete, 新路径自动启用)

## 裁剪点 (§1.2.1)

- L2 任务 (2 src 文件 + 1 test 文件, ~70 LOC src + ~400 LOC test)
- 按 §1.2.1 跳过 §14.5 深度审查, 走 §7.3 门审查 + §3.2 验收
- §3.2 验收全绿 (5633 tests, 0 failures, 0 clippy warnings)

## §3.2 验收

- `cargo fmt --check` ✓
- `cargo clippy --all-targets --features llvm-backend -- -D warnings` ✓ (0 warnings)
- `cargo test --release --features llvm-backend --lib` ✓ (898 tests, 0 failures, 0 ignored)
- `cargo test --release --features llvm-backend --test all_tests` ✓ (4735 tests, 0 failures, 9 ignored)

## Stage Summary

- TD-CODEGEN-CONST-SRC-TY-FROM-CONSTVAL 修复完成
- codegen operand.rs: 当 c.ty 为 concrete Int/Uint/Bool/Char 时用 emit_const_typed 直接 emit, 跳过 sext/trunc cast
- TextEmitter emit_const_typed contract 对齐 LLVM emitter (返回 raw value, 无 type prefix)
- 同时修复 Stage 18.287 遗留 bug (`store i64 i64 0` 双类型前缀 → `store i64 0`)
- 20 个新测试覆盖正向/text IR/负向/边界
- 架构健康度: 9.85/10 (stable — 2 src 文件, 无回归, 修复 +1 hidden bug)
- v0.644.0

## 下一步

- **Stage 110**: 重新引入 Phase 3.6 (Constant type writeback) — Stage 107 + Stage 109 已修复所有前置依赖 (call arg type source + codegen src_ty + TextEmitter contract)
- **Stage 111**: 加 Debug impl 验证 100 次跑 0 SIGSEGV — Phase 3.6 active 后 c.ty 全部 concrete, 验证非确定性 SIGSEGV 是否消除
- **Stage 112+**: 处理剩余 TD-TYPECK-WRITEBACK-INCOMPLETE 残留 (TD-MONO-INFER + TD-PRELUDE-IMPL-BODY-MODULE-ACCUMULATION)
