# Stage 128 开发日志 — TD-UFCS-SHORT-FORM (短形式 UFCS)

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.658.0 → v0.659.0 |
| 测试数 | 5785 (898 lib + 4887 integration, +17 stage128) |
| 失败数 | 0 |
| ignored | 9 |
| clippy warnings | 0 |
| fmt | clean |

## 5W2H

### WHAT
Stage 128 实现 UFCS 短形式 `Trait::method(receiver, args)` — Self 从 receiver 推断。

### WHY (根因分析)
- Stage 127 实现了完整形式 `<T as Trait>::method`，但短形式（Rust 推荐语法）缺失
- 根因：MIR lower 在解析 path 时无法访问 receiver（receiver 是 Call 的 args[0]）
- 通解：在 `lower_call_expr` 中，args lowering 后读取 receiver 类型，patch func_local 的 FnDef DefId + Assign 语句中的 Constant

### HOW (实施策略)
1. **Resolver**: `Trait::method` 2-segment 路径添加 `trait_method_index` 查找
2. **MIR lower `lower_call_expr`**:
   - 检测短形式（func 是 2-segment path, first segment 是 trait）
   - args lowering 后读取 receiver 类型
   - `resolve_ufcs_short_form_impl_method_def_id` 解析 impl 方法 DefId
   - **双 patch**: local_decl.ty + Assign 语句中的 Constant.ty + ConstVal
3. **共享逻辑**: `resolve_impl_method_by_name` 被 complete form + short form 复用

### 关键技术突破
- **根因发现**: typeck 的 `post_check_statement` unify place_ty (local_decl) 与 rvalue_ty (Constant)
- **双 patch 必要性**: 只 patch local_decl 不够 — Constant 的 ty 仍然是 trait DefId，导致 unify 失败 ("expected fn, found fn")
- **修复**: 扫描所有 basic_blocks 找到 Assign to func_local 的语句，patch Constant 的 ty + ConstVal

### HOW MUCH (§3.2 验收)
- cargo fmt --check ✓
- cargo clippy --all-targets --features llvm-backend -- -D warnings ✓ (0 warnings)
- cargo test --release --features llvm-backend --lib ✓ (898 tests, 0 failures)
- cargo test --release --features llvm-backend --test all_tests ✓ (4887 tests, 0 failures, 9 ignored)
- Total: 5785 tests, 0 failures, 9 ignored

## 决策点

### 决策点 1: 短形式检测位置
- **选 A**: `lower_call_expr` 中检测（args lowering 后 patch）— 能访问 receiver 类型
- **否决 B**: `lower_path_expr` 中检测 — 无法访问 receiver
- **依据**: §1.0 原则 9 (正确 > 妥协)

### 决策点 2: 双 patch 策略
- **选 A**: 同时 patch local_decl + Assign Constant — 保持一致性
- **否决 B**: 只 patch local_decl — 导致 typeck unify 失败
- **依据**: §1.0 原则 9 (正确 > 妥协) — 必须一致才能通过 typeck

## Stage Summary
- Stage 128 PASSED — 短形式 UFCS `Trait::method(receiver, args)` 完整实现
- 17 tests: 8 positive + 3 negative + 3 edge + 3 regression
- 0 regression (5768 → 5785 tests, +17 new)
- TD-UFCS-SHORT-FORM ✅ 已修复
- v0.659.0
