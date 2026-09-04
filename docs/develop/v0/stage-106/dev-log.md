# Stage 106 开发日志 — Constant type writeback 根因分析 (reverted)

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.642.0 (无版本变更 — RCA + revert) |
| 测试数 | 5613 (898 lib + 4715 integration) |
| 失败数 | 0 → 0 |
| ignored | 9 |
| clippy warnings | 0 |
| LOC 变更 | 0 (reverted) |

## 5W2H 根因分析

### WHAT (实验 + 发现)
尝试在 typeck Phase 3 后添加 Phase 3.6 (Constant type writeback): 遍历所有 statement + terminator, 对 `Operand::Constant(c).ty` 调用 `unify.resolve()` 替换 Infer 为 concrete Int/Uint。

### 结果
- **Infer warnings**: 18 → 0 ✓ (全部消除)
- **Constant type Infer warnings**: 8 → 0 ✓
- **回归**: 7 个 codegen 测试失败 (codegen_typed_call_args_i64 等)

### 根因 (WHY)
**BEFORE Stage 106**:
- Constant `42` 有 type `Infer(IntVar)` (lit_to_const 无 suffix → IntVar)
- typeck `default_unresolved()` 默认 IntVar → i32, 但 **不写 Constant.ty**
- codegen (TextEmitter) 对 call arg 用 **callee 的 sig** 确定 arg type → `i64`
- TextEmitter 输出: `call i64 @landin_g(i64 42)` — 正确!

**AFTER Stage 106**:
- Constant `42` 有 type `Int(I32)` (Phase 3.6 resolve Infer → i32)
- codegen (TextEmitter) 对 call arg 用 **Constant 的 type** → `i32`
- TextEmitter 输出: `sext i32 42 to i64` + `call i64 @landin_g(i64 %v2)` — 不同 IR 结构!

### 根因总结
codegen 对 call arg 的类型来源不一致:
- 当 Constant type 是 Infer → 用 callee sig (正确: i64)
- 当 Constant type 是 concrete → 用 Constant type (i32, 不是 i64)

**正确修复**: codegen 应该 **始终** 用 callee sig 确定 call arg type, 不论 Constant type 是 Infer 还是 concrete。

### 决策: Revert Stage 106

**选择**: Revert Stage 106 代码, 保留 plan + dev-log 作为 RCA 记录。

**理由** (§1.0 原则 9 正确>妥协):
- Constant type writeback 本身是正确的 (Infer 应该被 resolve)
- 但 codegen 对 call arg 的类型来源不正确 — 用 Constant type 而非 callee sig
- 正确修复需要改 codegen (不是 typeck), 这超出 Stage 106 范围
- 不在不完整的修复上妥协 → revert + 记录新 TD

### 新发现 TD

#### TD-CODEGEN-CALL-ARG-TYPE-SOURCE (P2, v0.12+)

**现象**: codegen 对 call arg 的类型来源不一致 — 当 Constant type 是 Infer 时用 callee sig (正确), 当 Constant type 是 concrete 时用 Constant type (可能错误)。

**根因**: codegen 的 `emit_call` / `codegen_operand` 路径对 call arg 用 `detect_operand_type` (读 Constant.ty) 而非 callee sig。当 Constant.ty 是 Infer 时 fallback 到 callee sig; 当是 concrete 时直接用。

**修复方案**: codegen 的 call arg 类型应始终来自 callee sig, 不来自 Constant.ty。

**影响**: 修复后 Stage 106 的 Constant type writeback 可以重新引入, 不会产生回归。

### §3.2 验收

- `cargo fmt --check` ✓
- `cargo clippy --all-targets --features llvm-backend -- -D warnings` ✓ (0 warnings)
- `cargo test --release --features llvm-backend --lib` ✓ (898 tests, 0 failures, 0 ignored)
- `cargo test --release --features llvm-backend --test all_tests` ✓ (4715 tests, 0 failures, 9 ignored)

## 下一步

- **Stage 107**: 修复 TD-CODEGEN-CALL-ARG-TYPE-SOURCE — codegen call arg 类型始终来自 callee sig
- **Stage 108**: 重新引入 Stage 106 的 Constant type writeback (Phase 3.6)
- **Stage 109**: 加 Debug impl 验证 100 次跑 0 SIGSEGV
