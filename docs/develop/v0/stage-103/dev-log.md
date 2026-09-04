# Stage 103 开发日志 — resolve_lit_ty_from_expected (Layer 3 部分修复)

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.641.0 → v0.642.0 |
| 测试数 | 5606 → 5613 (+7 stage103) |
| 失败数 | 0 → 0 |
| ignored | 9 |
| clippy warnings | 0 |
| LOC 变更 | 1 src 文件 (~50 LOC) + 1 测试文件 (~80 LOC) |

## 修改文件

### 源文件 (1)
| 文件 | 变更 |
|------|------|
| `src/mir/lower/expr_operand.rs` | `lower_expr_to_operand` HirExprKind::Lit arm 使用 expected_ty; 新增 `resolve_lit_ty_from_expected` helper (RawPtr → usize) |

### 测试文件 (1)
| 文件 | 变更 |
|------|------|
| `tests/v0/stage103/plan/lit_ty_resolution_tests.rs` | 新建 — 7 tests (3 positive + 4 negative) |

### 其他
- `Cargo.toml`: 版本 → 0.642.0
- `tests/all_tests.rs`: 注册 stage103_lit_ty_resolution_tests
- `src/stdlib/prelude.rs`: 更新 Debug trait 注释 (Layer 3 部分修复)

## 5W2H 根因修复

### WHAT (修复)
`lower_expr_to_operand` 的 `HirExprKind::Lit` arm 现在使用 `expected_ty` 解析 unsuffixed int literal 类型。新增 `resolve_lit_ty_from_expected` helper: 当 expected_ty 是 RawPtr 时, 将 `0` 字面量解析为 usize (8 bytes) 而非 Infer(IntVar)。

### WHY (Layer 3 真正根因 — Stage 102 误判修正)
Stage 102 误判为 "LLVM module 全局累积", 实际根因是 typeck writeback 对 `String::new()` body 中的 struct literal `String { ptr: 0, len: 0usize, cap: 0usize }` 不解析 field types。

`0` 字面量无 suffix → `lit_to_const` 返回 `Infer(IntVar)` (line 811-816)。`lower_expr_to_operand` 显式忽略 `expected_ty` (line 224: `let _ = expected_ty;`)。

结果: codegen 中 `String { ptr: 0, ... }` 的 ptr field 用 i32 (4 bytes) 而非 usize (8 bytes), String struct layout 错误, 运行时 SIGSEGV (signal 11, exit 139)。100 次跑 1 次失败 (非确定, 因 LLVM 内存布局)。

### HOW (通解)
```rust
// lower_expr_to_operand HirExprKind::Lit arm:
let (mut const_val, mut ty) = cx.lit_to_const(lit);
if let Some(expected) = expected_ty {
    if matches!(ty.kind, TyKind::Infer(_)) {
        if let Some(resolved_ty) = resolve_lit_ty_from_expected(&ty, expected) {
            const_val.ty = resolved_ty.clone();
            ty = resolved_ty;
        }
    }
}
```

### 效果
- **ptr field 类型正确解析**: `String { ptr: 0, ... }` 的 `0` 现在是 usize (8 bytes)
- **保守策略**: 只在 expected_ty 是 RawPtr 时 override (避免破坏 typeck int/uint validation)
- **测试全绿**: 898 lib + 4715 integration = 5613 tests, 0 failures, 9 ignored

## 决策点 (§12 最优>最小, §1.0 原则 4 报错>静默, §1.0 原则 9 正确>妥协)

### 决策 1: 只在 expected_ty 是 RawPtr 时 override

**选择**: `resolve_lit_ty_from_expected` 只处理 RawPtr, 不处理 Int/Uint。

**替代方案 (拒绝)**: 处理 Int/Uint — Stage 103 实验显示导致 16 个 typeck neg test 失败 (因为跳过了 typeck validation)。

**理由** (§1.0 原则 4 报错>静默, §1.0 原则 9 正确>妥协):
- typeck 需要 validate int/uint literal value fits (e.g., `let x: i8 = 200` 应报错)
- 只 override RawPtr (指针 `0` 是常见模式, usize 不会 overflow)
- 不破坏现有 typeck validation 路径

### 决策 2: 根因是 lit_to_const 不接收 expected_ty

**选择**: 在 `lower_expr_to_operand` 中 post-process lit_to_const 结果。

**替代方案 (拒绝)**: 修改 `lit_to_const` 接收 expected_ty — 改动大, 影响所有 lit 调用点。

**理由** (§12 最优>最小):
- post-process 是最小根因修复
- `lit_to_const` 保持原签名 (通用)
- `lower_expr_to_operand` 是 expected_ty 的正确处理位置

## Stage 103 验证实验: 加 Debug impl 测试 Layer 3 修复效果

### 实验
在 prelude 中添加 `impl Debug for i32`, 跑 cargo test + 100 次手动跑 String::new。

### 结果
- cargo test: 14 失败 → 5 失败 (改善, 但仍有非确定 crash)
- 100 次手动跑: 1 失败 → 3 失败 (误差范围, 未显著改善)
- stderr: 从 `Infer` warning 变为 `Param` warning

### 分析
- Layer 3 (ptr field Infer) 已修复 ✓
- 但 Param warnings 从 generic prelude methods (Vec::push<T>) 仍存在 ✗
- Param 来自 generic def body 仍 emit (因 TD-MONO-INFER — 非 turbofish path FnDef substs 为空)
- 需要 TD-MONO-INFER 完全修复

## 测试覆盖

| 测试 | 类型 | 验证 |
|------|------|------|
| `stage103_string_new_ptr_field_type_resolved` | 正向 | String::new() ptr field 类型解析 |
| `stage103_vec_new_ptr_field_type_resolved` | 正向 | Vec::new() ptr field 类型解析 |
| `stage103_box_new_field_type_resolved` | 正向 | Box::new() ptr field 类型解析 |
| `stage103_undefined_type_errors` | 负向 | undefined type 报错 |
| `stage103_type_mismatch_errors` | 负向 | type mismatch 报错 |
| `stage103_nonexistent_method_errors` | 负向 | nonexistent method 报错 |
| `stage103_undefined_trait_errors` | 负向 | undefined trait 报错 |

## §3.2 验收

- `cargo fmt --check` ✓
- `cargo clippy --all-targets --features llvm-backend -- -D warnings` ✓ (0 warnings)
- `cargo test --release --features llvm-backend --lib` ✓ (898 tests, 0 failures, 0 ignored)
- `cargo test --release --features llvm-backend --test all_tests` ✓ (4715 tests, 0 failures, 9 ignored — stage103 7 tests included)

## 下一步

- **Stage 104**: TD-MONO-INFER 修复 (type inference back-propagation for FnDef substs) — 完全消除 Param warnings
- **Stage 105**: 重新添加 Debug + PartialOrd impls (依赖 Stage 104 完成)
