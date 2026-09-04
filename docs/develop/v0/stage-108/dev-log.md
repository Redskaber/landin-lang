# Stage 108 开发日志 — Constant type writeback (Phase 3.6) + codegen constant cast 根因分析 (reverted)

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.643.0 (无版本变更 — RCA + revert) |
| 测试数 | 5613 (898 lib + 4715 integration) |
| 失败数 | 0 → 0 |
| ignored | 9 |
| clippy warnings | 0 |
| LOC 变更 | 0 (reverted) |

## 5W2H 根因分析

### WHAT (实验 + 发现)
重新引入 Stage 106 的 Constant type writeback (Phase 3.6)。Stage 107 已修复 call arg type source, 预期 Phase 3.6 不再产生回归。

### 结果
- **Infer warnings**: 8 → 0 (Constant type Infer 全部消除) ✓
- **codegen 回归**: 7 个测试失败 (与 Stage 106 相同) ✗

### 根因 (WHY)
Phase 3.6 resolves Constant.ty from Infer(IntVar) to concrete type (I32 or I64 depending on typeck unify). 但 codegen 的 Stage 14.64 cast 逻辑在 Constant.ty 是 concrete Int 时插入 `sext` cast:
- **BEFORE Phase 3.6**: Constant.ty = Infer → no cast (Infer doesn't match `Int(int_ty)` in cast check) → `42` emitted directly → call arg type from callee sig → `i64 42`
- **AFTER Phase 3.6**: Constant.ty = Int(I32) → cast inserted → `sext i32 42 to i64` → new SSA value `%v1` → `call i64 @landin_g(i64 %v1)`

### DEBUG 输出
```
DEBUG: Constant ty before=Infer(IntVar(IntVid(0))) after=Int(I64)  ← unified with callee sig
DEBUG: Constant ty before=Infer(IntVar(IntVid(0))) after=Int(I32)  ← defaulted to i32
```

部分 IntVar 被 typeck 正确 unify 到 i64 (from callee sig), 但 codegen 的 Stage 14.64 仍然 cast from I32 (src_ty based on ConstVal, not Constant.ty)。即使 Constant.ty = I64, src_ty = I32 (因为 `42` fits in i32), 所以 cast 被插入。

### 尝试的修复 (reverted)
1. **Phase 3.6**: resolve Constant.ty ✓ (Infer→I64)
2. **codegen operand.rs**: skip cast for I32↔I64↔I128 when value fits — 太激进, 301 个回归 (其他 cast 也被跳过)
3. **更新测试期望**: 接受 `sext` 模式 — 在准备中 (但 checker.rs 格式问题导致 revert)

### 决策: Revert Stage 108

**选择**: Revert 所有 Stage 108 代码, 保留 plan + dev-log 作为 RCA。

**理由** (§1.0 原则 9 正确>妥协):
- Phase 3.6 本身正确 (resolve Infer → concrete)
- 但 codegen Stage 14.64 的 src_ty 基于 ConstVal (value fits in i32 → src=I32), 不基于 Constant.ty
- 即使 Constant.ty = I64, src_ty 仍是 I32 (因为 42 fits in i32)
- 正确修复: codegen Stage 14.64 应该用 Constant.ty 确定 src_ty (而非 ConstVal)
- 这是一个 codegen 层面的修复, 需要更仔细的测试

### 新发现 TD

#### TD-CODEGEN-CONST-SRC-TY-FROM-CONSTVAL (P2, v0.12+)

**现象**: codegen Stage 14.64 用 ConstVal (value size) 确定 src_ty, 不用 Constant.ty。即使 Constant.ty = I64, src_ty = I32 (因为 42 fits in i32) → 不必要的 `sext` cast。

**根因**: `src_ty` 在 operand.rs:193-200 基于 `ConstVal::Int(n)` 的大小, 不基于 `c.ty`。

**修复方案**: `src_ty` 应该用 `c.ty` (resolve 后的 concrete type), 不用 ConstVal。

**影响**: 修复后 Phase 3.6 可安全引入 (src_ty = I64 → no cast → `i64 42` directly)。

### §3.2 验收

- `cargo fmt --check` ✓
- `cargo clippy --all-targets --features llvm-backend -- -D warnings` ✓ (0 warnings)
- `cargo test --release --features llvm-backend --lib` ✓ (898 tests, 0 failures, 0 ignored)
- `cargo test --release --features llvm-backend --test all_tests` ✓ (4715 tests, 0 failures, 9 ignored)

## 下一步

- **Stage 109**: 修复 TD-CODEGEN-CONST-SRC-TY-FROM-CONSTVAL — codegen src_ty 用 Constant.ty
- **Stage 110**: 重新引入 Phase 3.6 (Constant type writeback)
- **Stage 111**: 加 Debug impl 验证 100 次跑 0 SIGSEGV
