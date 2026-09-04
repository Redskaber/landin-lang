# Stage 107 开发日志 — TD-CODEGEN-CALL-ARG-TYPE-SOURCE 修复

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.642.0 → v0.643.0 |
| 测试数 | 5613 (898 lib + 4715 integration) |
| 失败数 | 0 → 0 |
| ignored | 9 |
| clippy warnings | 0 |
| LOC 变更 | 1 src 文件 (~60 LOC) |

## 修改文件

### 源文件 (1)
| 文件 | 变更 |
|------|------|
| `src/codegen/terminator.rs` | Call terminator arg type: 优先用 callee sig.inputs (非 Param 时); 新增 `mir_type_contains_param` helper |

## 5W2H 根因修复

### WHAT (修复)
Call terminator 的 arg type 从 `detect_operand_type` (读 Constant.ty) 改为优先用 callee sig.inputs[arg_idx]。当 callee sig 含 Param (generic function) 时 fallback 到 `detect_operand_type`。

### WHY (根因)
Stage 106 发现: Constant type writeback (Phase 3.6) 导致 7 个 codegen 回归。根因: codegen call arg type source 不一致:
- 当 Constant.ty 是 Infer → `detect_operand_type` fallback 到 I32 (恰好匹配某些 callee sig)
- 当 Constant.ty 被 resolve 为 I32 → `detect_operand_type` 返回 I32 (即使 callee 期望 i64)

### HOW (通解)
```rust
let callee_param_ty = callee_def_id_early
    .and_then(|did| fn_sigs.get(&did))
    .and_then(|sig| sig.inputs.get(arg_idx));
let ty = if let Some(param_ty) = callee_param_ty {
    if !mir_type_contains_param(param_ty) {
        // Non-generic callee — use sig type (authoritative)
        mir_type_to_emit_type_with_layouts_and_mono(param_ty, ...)
    } else {
        // Generic callee — fallback to operand type (resolved by typeck/writeback)
        detect_operand_type(mir, a, ...)
    }
} else {
    // No callee sig — fallback to operand type
    detect_operand_type(mir, a, ...)
};
```

### 决策点 (§12 最优>最小, §1.0 原则 6 通解>特解)

#### 决策 1: 优先用 callee sig, 但 generic 时 fallback

**选择**: 当 callee sig param type 不含 Param 时用 sig; 含 Param 时 fallback 到 operand type。

**理由** (§1.0 原则 6 通解>特解, §1.0 原则 9 正确>妥协):
- Non-generic callee: sig 是权威类型来源 (e.g., `fn g(a: i64)` → arg type = i64)
- Generic callee: sig 含 Param(N), 不能直接用 (Param → I32 fallback 错误)
- Generic callee 的 arg type 由 typeck/writeback 已 resolve 到 operand 的 local_decl.ty

#### 决策 2: 新增 `mir_type_contains_param` helper

**选择**: 在 `terminator.rs` 中新增 `pub(crate) fn mir_type_contains_param`。

**理由** (§1.0 原则 6 通解>特解):
- `type_contains_param_recursive` 在 `typeck/check.rs` 中是 private
- 新增独立 helper 遵循 §16 接口隔离 (codegen 不依赖 typeck 内部函数)
- 递归处理所有 type kinds (Param/Ref/RawPtr/Slice/Array/Tuple/Adt/Closure/FnDef)

## §3.2 验收

- `cargo fmt --check` ✓
- `cargo clippy --all-targets --features llvm-backend -- -D warnings` ✓ (0 warnings)
- `cargo test --release --features llvm-backend --lib` ✓ (898 tests, 0 failures, 0 ignored)
- `cargo test --release --features llvm-backend --test all_tests` ✓ (4715 tests, 0 failures, 9 ignored)

## 下一步

- **Stage 108**: 重新引入 Stage 106 的 Constant type writeback (Phase 3.6) — Stage 107 已修复 call arg type source, Phase 3.6 不再产生回归
- **Stage 109**: 加 Debug impl 验证 100 次跑 0 SIGSEGV
